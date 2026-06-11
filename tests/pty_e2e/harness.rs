//! PTY end-to-end test harness for reedline.
//!
//! Spawns the `pty_fixture` example (a deterministic reedline REPL, see
//! `examples/pty_fixture.rs`) inside a real pseudo-terminal, feeds its output
//! through an in-memory VT100 screen, and lets tests make Neovim-style
//! "eventual screen state" assertions: every `expect_*` helper retries until
//! the screen matches or a timeout expires, so tests never sleep.
//!
//! The harness also acts as the "terminal side" of cursor-position queries:
//! when the child emits `CSI 6n` (DSR), the harness replies with the emulated
//! cursor position and counts the query, so tests can assert both rendering
//! *and* how many round-trips the painter performed.

use std::io::{Read, Write};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, Once};
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

use crate::keys::keys_to_chunks;

const DSR_QUERY: &[u8] = b"\x1b[6n";

/// Scrollback kept by the emulated screen; tests assert on the visible grid
/// only, scrollback just avoids losing context in failure dumps.
const SCROLLBACK: usize = 100;

fn base_timeout() -> Duration {
    let ms = std::env::var("PTY_E2E_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5_000u64);
    let scale = if std::env::var_os("CI").is_some() {
        3
    } else {
        1
    };
    Duration::from_millis(ms * scale)
}

struct ScreenState {
    parser: vt100::Parser,
    dsr_count: usize,
    eof: bool,
    /// Bumped once per chunk of child output; lets waiters detect activity.
    generation: u64,
}

struct Shared {
    state: Mutex<ScreenState>,
    cond: Condvar,
}

/// A reedline fixture running inside a real PTY.
pub struct TestTerm {
    shared: Arc<Shared>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    reader_thread: Option<std::thread::JoinHandle<()>>,
}

pub struct TestTermBuilder {
    rows: u16,
    cols: u16,
    envs: Vec<(String, String)>,
}

impl TestTermBuilder {
    pub fn size(mut self, rows: u16, cols: u16) -> Self {
        self.rows = rows;
        self.cols = cols;
        self
    }

    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.envs.push((key.into(), value.into()));
        self
    }

    /// Left prompt text; `\n` (escaped) becomes a real newline.
    pub fn prompt(self, text: &str) -> Self {
        self.env("FIX_PROMPT", text)
    }

    pub fn right_prompt(self, text: &str) -> Self {
        self.env("FIX_RIGHT_PROMPT", text)
    }

    /// Seed history entries, oldest first.
    pub fn history(self, entries: &[&str]) -> Self {
        self.env("FIX_HISTORY", &entries.join("\n"))
    }

    pub fn vi_mode(self) -> Self {
        self.env("FIX_VI", "1")
    }

    pub fn completion_menu(self) -> Self {
        self.env("FIX_MENU", "1")
    }

    pub fn highlighter(self) -> Self {
        self.env("FIX_HIGHLIGHT", "1")
    }

    /// Shell snippet run as the buffer editor (`Ctrl-O`); `$1` is the file
    /// holding the buffer.
    pub fn editor_cmd(self, snippet: &str) -> Self {
        self.env("FIX_EDITOR_CMD", snippet)
    }

    pub fn spawn(self) -> TestTerm {
        let fixture = ensure_fixture_built();

        let pty = native_pty_system()
            .openpty(PtySize {
                rows: self.rows,
                cols: self.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("open pty");

        let mut cmd = CommandBuilder::new(fixture);
        cmd.env("TERM", "xterm-256color");
        cmd.cwd(env!("CARGO_MANIFEST_DIR"));
        for (k, v) in &self.envs {
            cmd.env(k, v);
        }

        let child = pty.slave.spawn_command(cmd).expect("spawn fixture");
        // Drop the slave so the master sees EOF when the child exits.
        drop(pty.slave);

        let reader = pty.master.try_clone_reader().expect("clone pty reader");
        let writer: Arc<Mutex<Box<dyn Write + Send>>> =
            Arc::new(Mutex::new(pty.master.take_writer().expect("pty writer")));

        let shared = Arc::new(Shared {
            state: Mutex::new(ScreenState {
                parser: vt100::Parser::new(self.rows, self.cols, SCROLLBACK),
                dsr_count: 0,
                eof: false,
                generation: 0,
            }),
            cond: Condvar::new(),
        });

        let reader_thread = {
            let shared = Arc::clone(&shared);
            let writer = Arc::clone(&writer);
            std::thread::spawn(move || pump_output(reader, &shared, &writer))
        };

        let term = TestTerm {
            shared,
            writer,
            master: pty.master,
            child,
            reader_thread: Some(reader_thread),
        };

        // Block until the first prompt is painted. Input sent before the
        // fixture enables raw mode would be echoed by the line discipline
        // and pollute the screen, so no test may race the startup.
        let marker = self
            .envs
            .iter()
            .find(|(k, _)| k == "FIX_PROMPT")
            .map(|(_, v)| v.replace("\\n", "\n"))
            .unwrap_or_else(|| "tst> ".into())
            .lines()
            .last()
            .unwrap_or("tst> ")
            .trim_end()
            .to_string();
        term.expect("initial prompt", move |screen| {
            let (row, _) = screen.cursor_position();
            let text = screen_rows(screen)
                .get(row as usize)
                .cloned()
                .unwrap_or_default();
            if text.starts_with(&marker) {
                Ok(())
            } else {
                Err(format!("cursor row reads {text:?}"))
            }
        });
        term
    }
}

/// Reads child output, answers DSR (`CSI 6n`) cursor-position queries with
/// the emulated cursor position, and feeds everything else to the VT100
/// screen.
fn pump_output(
    mut reader: Box<dyn Read + Send>,
    shared: &Shared,
    writer: &Mutex<Box<dyn Write + Send>>,
) {
    let mut pending: Vec<u8> = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) | Err(_) => {
                let mut state = shared
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if !pending.is_empty() {
                    state.parser.process(&pending);
                }
                state.eof = true;
                state.generation += 1;
                shared.cond.notify_all();
                return;
            }
            Ok(n) => n,
        };
        pending.extend_from_slice(&buf[..n]);

        // Hold back a trailing partial DSR query so it isn't half-processed;
        // it completes (or turns out to be something else) on the next read.
        let hold = partial_dsr_suffix(&pending);
        let workable: Vec<u8> = pending[..pending.len() - hold].to_vec();
        pending.drain(..workable.len());

        let mut state = shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut rest = workable.as_slice();
        while let Some(at) = find(rest, DSR_QUERY) {
            state.parser.process(&rest[..at]);
            state.dsr_count += 1;
            let (row, col) = state.parser.screen().cursor_position();
            let reply = format!("\x1b[{};{}R", row + 1, col + 1);
            {
                let mut w = writer
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                w.write_all(reply.as_bytes()).ok();
                w.flush().ok();
            }
            rest = &rest[at + DSR_QUERY.len()..];
        }
        state.parser.process(rest);
        state.generation += 1;
        drop(state);
        shared.cond.notify_all();
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Length of the longest proper prefix of `CSI 6n` that `bytes` ends with.
fn partial_dsr_suffix(bytes: &[u8]) -> usize {
    for len in (1..DSR_QUERY.len()).rev() {
        if bytes.ends_with(&DSR_QUERY[..len]) {
            return len;
        }
    }
    0
}

impl TestTerm {
    /// A 24x80 terminal running the fixture with default settings.
    pub fn spawn() -> Self {
        Self::builder().spawn()
    }

    pub fn builder() -> TestTermBuilder {
        TestTermBuilder {
            rows: 24,
            cols: 80,
            envs: Vec::new(),
        }
    }

    // ---- input ----------------------------------------------------------

    /// Send input using key notation: literal text plus `<...>` tokens, e.g.
    /// `"abc<Left><C-w><Enter>"`. See `keys.rs` for the token list.
    ///
    /// A bare `<Esc>` forces a short pause before any following bytes so the
    /// terminal-side parser sees a standalone Escape key press rather than
    /// an Alt-chord.
    pub fn send(&self, keys: &str) {
        let chunks = keys_to_chunks(keys);
        for (i, chunk) in chunks.iter().enumerate() {
            if i > 0 {
                std::thread::sleep(Duration::from_millis(50));
            }
            if !chunk.is_empty() {
                self.send_raw(chunk);
            }
        }
    }

    pub fn send_raw(&self, bytes: &[u8]) {
        let mut writer = self
            .writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        writer.write_all(bytes).expect("write to pty");
        writer.flush().expect("flush pty");
    }

    /// Send `text` wrapped in a bracketed-paste envelope.
    pub fn paste(&self, text: &str) {
        let mut bytes = b"\x1b[200~".to_vec();
        bytes.extend_from_slice(text.as_bytes());
        bytes.extend_from_slice(b"\x1b[201~");
        self.send_raw(&bytes);
    }

    /// Resize the PTY (delivers SIGWINCH to the child) and the emulated
    /// screen together.
    pub fn resize(&self, rows: u16, cols: u16) {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("resize pty");
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.parser.screen_mut().set_size(rows, cols);
        state.generation += 1;
        drop(state);
        self.shared.cond.notify_all();
    }

    // ---- queries ---------------------------------------------------------

    /// Number of `CSI 6n` cursor-position queries the child has issued.
    pub fn dsr_count(&self) -> usize {
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .dsr_count
    }

    /// Current visible screen as text, rows right-trimmed, for debugging.
    pub fn screen_text(&self) -> String {
        let state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        screen_rows(state.parser.screen()).join("\n")
    }

    // ---- assertions ------------------------------------------------------

    /// Core eventual-state assertion: re-evaluates `check` after every chunk
    /// of child output until it returns `Ok` or the timeout expires.
    ///
    /// Modeled on Neovim's `Screen:expect()`: asserting on the *eventual*
    /// state makes tests immune to repaint timing, and the failure message
    /// reports the last mismatch plus a screen dump.
    pub fn expect(&self, what: &str, check: impl Fn(&vt100::Screen) -> Result<(), String>) {
        let deadline = Instant::now() + base_timeout();
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            let last_err = match check(state.parser.screen()) {
                Ok(()) => return,
                Err(err) => err,
            };
            let now = Instant::now();
            if now >= deadline || state.eof {
                let eof_note = if state.eof {
                    "\n(child exited; output is final)"
                } else {
                    ""
                };
                panic!(
                    "timed out waiting for {what}: {last_err}{eof_note}\n--- screen ---\n{}\n--------------",
                    dump_screen(state.parser.screen()),
                );
            }
            let (next, _) = self
                .shared
                .cond
                .wait_timeout(state, deadline - now)
                .unwrap();
            state = next;
        }
    }

    /// Assert the full visible screen equals `expected`.
    ///
    /// `expected` lists rows top to bottom; rows are compared right-trimmed
    /// and missing trailing rows must be blank, so tests only write the
    /// interesting part of the screen:
    ///
    /// ```text
    /// term.expect_screen("\
    /// tst> hello
    /// GOT: hello
    /// tst> ");
    /// ```
    pub fn expect_screen(&self, expected: &str) {
        let want: Vec<String> = expected.lines().map(|l| l.trim_end().to_string()).collect();
        self.expect("screen to match", move |screen| {
            let got = screen_rows(screen);
            let mut want = want.clone();
            while want.len() < got.len() {
                want.push(String::new());
            }
            if got == want {
                Ok(())
            } else {
                Err("contents differ".to_string())
            }
        });
    }

    /// Assert row `row` (0-based) equals `expected` after right-trimming.
    pub fn expect_line(&self, row: u16, expected: &str) {
        let expected = expected.trim_end().to_string();
        self.expect(&format!("row {row} to be {expected:?}"), move |screen| {
            let got = row_text(screen, row);
            if got == expected {
                Ok(())
            } else {
                Err(format!("row {row} is {got:?}"))
            }
        });
    }

    /// Assert `needle` appears somewhere on the visible screen.
    pub fn expect_contains(&self, needle: &str) {
        let needle = needle.to_string();
        self.expect(&format!("screen to contain {needle:?}"), move |screen| {
            if screen_rows(screen).iter().any(|row| row.contains(&needle)) {
                Ok(())
            } else {
                Err("not found".to_string())
            }
        });
    }

    pub fn expect_cursor(&self, row: u16, col: u16) {
        self.expect(&format!("cursor at ({row}, {col})"), move |screen| {
            let got = screen.cursor_position();
            if got == (row, col) {
                Ok(())
            } else {
                Err(format!("cursor at {got:?}"))
            }
        });
    }

    /// Assert the foreground color of every cell of `row` within `cols`.
    pub fn expect_fg(&self, row: u16, cols: Range<u16>, color: vt100::Color) {
        self.expect(
            &format!("fg color {color:?} at row {row}, cols {cols:?}"),
            move |screen| {
                for col in cols.clone() {
                    let cell = screen
                        .cell(row, col)
                        .ok_or_else(|| format!("no cell at ({row}, {col})"))?;
                    if cell.fgcolor() != color {
                        return Err(format!(
                            "cell ({row}, {col}) {:?} has fg {:?}",
                            cell.contents(),
                            cell.fgcolor()
                        ));
                    }
                }
                Ok(())
            },
        );
    }

    /// Assert the screen does NOT change for `window` (Neovim's `unchanged`).
    /// Any output that alters the visible screen within the window fails.
    pub fn expect_unchanged(&self, window: Duration) {
        let deadline = Instant::now() + window;
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let baseline = screen_rows(state.parser.screen());
        loop {
            let now = Instant::now();
            if now >= deadline {
                return;
            }
            let (next, _) = self
                .shared
                .cond
                .wait_timeout(state, deadline - now)
                .unwrap();
            state = next;
            let current = screen_rows(state.parser.screen());
            assert_eq!(
                baseline,
                current,
                "screen changed during an expect_unchanged window\n--- screen ---\n{}",
                dump_screen(state.parser.screen())
            );
        }
    }

    // ---- lifecycle -------------------------------------------------------

    /// Abort any in-progress buffer with Ctrl-C, then `quit`.
    pub fn quit_after_clear(self) {
        self.send("<C-c>");
        self.expect_contains("(ctrl-c)");
        self.quit();
    }

    /// Gracefully stop the fixture (assumes an empty prompt) and assert it
    /// exits cleanly.
    pub fn quit(mut self) {
        self.send(":quit<Enter>");
        let deadline = Instant::now() + base_timeout();
        loop {
            if let Some(status) = self.child.try_wait().expect("wait for fixture") {
                assert!(status.success(), "fixture exited with {status:?}");
                break;
            }
            if Instant::now() >= deadline {
                panic!(
                    "fixture did not exit after :quit\n--- screen ---\n{}",
                    self.screen_text()
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for TestTerm {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(handle) = self.reader_thread.take() {
            let _ = handle.join();
        }
    }
}

// ---- screen helpers -------------------------------------------------------

/// Visible rows as right-trimmed text; wide-cell continuations are skipped so
/// emoji occupy one string position per glyph. Public so tests can write
/// custom `expect` predicates over plain text rows.
pub fn screen_rows(screen: &vt100::Screen) -> Vec<String> {
    let (rows, cols) = screen.size();
    (0..rows)
        .map(|row| {
            let mut text = String::new();
            for col in 0..cols {
                match screen.cell(row, col) {
                    Some(cell) if cell.is_wide_continuation() => {}
                    Some(cell) if cell.contents().is_empty() => text.push(' '),
                    Some(cell) => text.push_str(&cell.contents()),
                    None => text.push(' '),
                }
            }
            text.trim_end().to_string()
        })
        .collect()
}

fn row_text(screen: &vt100::Screen, row: u16) -> String {
    screen_rows(screen)
        .into_iter()
        .nth(row as usize)
        .unwrap_or_default()
}

/// Render the screen with a border and cursor marker for failure messages.
fn dump_screen(screen: &vt100::Screen) -> String {
    let (rows, cols) = screen.size();
    let (cur_row, cur_col) = screen.cursor_position();
    let mut out = String::new();
    out.push_str(&format!(
        "size {rows}x{cols}, cursor ({cur_row}, {cur_col})\n"
    ));
    out.push_str(&format!("+{}+\n", "-".repeat(cols as usize)));
    for (i, row) in screen_rows(screen).iter().enumerate() {
        let mut padded = format!("{row:<width$}", width = cols as usize);
        padded.truncate(cols as usize);
        out.push_str(&format!("|{padded}| {i}\n"));
    }
    out.push_str(&format!("+{}+", "-".repeat(cols as usize)));
    out
}

// ---- fixture build --------------------------------------------------------

static FIXTURE_BUILD: Once = Once::new();

/// Build the `pty_fixture` example (idempotent; cargo no-ops when fresh) and
/// return its path. Tests run after `cargo test`/`cargo nextest run` has
/// released the build lock, so invoking cargo here is safe; concurrent test
/// processes serialize on cargo's own lock.
fn ensure_fixture_built() -> PathBuf {
    let exe = std::env::current_exe().expect("test exe path");
    // target/<profile>/deps/<test-bin> -> target/<profile>
    let profile_dir = exe
        .parent()
        .and_then(|deps| deps.parent())
        .expect("locate target profile dir")
        .to_path_buf();
    let fixture = profile_dir.join("examples").join("pty_fixture");

    FIXTURE_BUILD.call_once(|| {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let mut cmd = std::process::Command::new(cargo);
        cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
        cmd.args(["build", "--example", "pty_fixture"]);
        if cfg!(feature = "external_printer") {
            cmd.args(["--features", "external_printer"]);
        }
        if profile_dir.file_name().is_some_and(|n| n == "release") {
            cmd.arg("--release");
        }
        let status = cmd.status().expect("run cargo build for pty_fixture");
        assert!(status.success(), "building the pty_fixture example failed");
    });

    assert!(
        fixture.exists(),
        "fixture binary missing at {}",
        fixture.display()
    );
    fixture
}
