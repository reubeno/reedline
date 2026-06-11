//! PTY end-to-end test harness for reedline.
//!
//! Spawns the `e2e_fixture` example (a deterministic reedline REPL, see
//! `examples/e2e_fixture.rs`) inside a real pseudo-terminal, feeds its output
//! through an in-memory VT100 screen, and lets tests make Neovim-style
//! "eventual screen state" assertions: every `expect_*` helper retries until
//! the screen matches or a timeout expires, so tests never sleep to wait
//! for output. (The one timing concession is on the *input* side: a short
//! pause after a bare `<Esc>`, see [`TestTerm::send`].)
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

/// All timing in the harness derives from these two knobs so CI load is
/// compensated in one place.
fn time_scale() -> u64 {
    if std::env::var_os("CI").is_some() {
        3
    } else {
        1
    }
}

fn base_timeout() -> Duration {
    let ms = std::env::var("E2E_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5_000u64);
    Duration::from_millis(ms * time_scale())
}

/// Pause inserted after a bare `<Esc>` so the terminal-side parser sees a
/// standalone Escape rather than the start of an Alt-chord.
fn esc_pause() -> Duration {
    Duration::from_millis(50 * time_scale())
}

/// Window for `expect_unchanged`-style negative assertions.
fn unchanged_window() -> Duration {
    Duration::from_millis(200 * time_scale())
}

/// Colors the fixture paints with, as seen by the emulated screen. Keep in
/// sync with `examples/e2e_fixture.rs` (highlighter / hinter styles) so the
/// fixture and assertions agree in one place.
pub const RED: vt100::Color = vt100::Color::Idx(1);
pub const GREEN: vt100::Color = vt100::Color::Idx(2);
pub const DARK_GRAY: vt100::Color = vt100::Color::Idx(8);

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
    /// Last line of the configured prompt, right-trimmed (e.g. "tst>"); used
    /// to recognize a fresh prompt row.
    prompt_marker: String,
    /// Flag file watched by the fixture's break-signal thread, if enabled.
    break_flag: Option<PathBuf>,
}

pub struct TestTermBuilder {
    rows: u16,
    cols: u16,
    envs: Vec<(String, String)>,
    break_flag: Option<PathBuf>,
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

    /// Enable the host-driven break signal; trigger it from a test with
    /// [`TestTerm::trigger_break`].
    pub fn break_signal(mut self) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "reedline-e2e-break-{}-{unique}.flag",
            std::process::id(),
        ));
        let _ = std::fs::remove_file(&path);
        self.break_flag = Some(path.clone());
        self.env("FIX_BREAK_FLAG", path.to_str().expect("utf-8 temp path"))
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

        let marker = strip_csi(
            self.envs
                .iter()
                .find(|(k, _)| k == "FIX_PROMPT")
                .map(|(_, v)| v.replace("\\n", "\n"))
                .unwrap_or_else(|| "tst> ".into())
                .lines()
                .last()
                .unwrap_or("tst> "),
        )
        .trim_end()
        .to_string();

        let term = TestTerm {
            shared,
            writer,
            master: pty.master,
            child,
            reader_thread: Some(reader_thread),
            prompt_marker: marker.clone(),
            break_flag: self.break_flag,
        };

        // Block until the first prompt is painted. Input sent before the
        // fixture enables raw mode would be echoed by the line discipline
        // and pollute the screen, so no test may race the startup. A prompt
        // wider than the screen wraps, so the cursor row holding only a
        // suffix of the prompt also counts.
        term.expect("initial prompt", move |screen| {
            let (row, _) = screen.cursor_position();
            let text = screen_rows(screen)
                .get(row as usize)
                .cloned()
                .unwrap_or_default();
            if text.starts_with(&marker) || (!text.is_empty() && marker.ends_with(text.trim_end()))
            {
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

/// Remove CSI escape sequences (`ESC [ ... final-byte`), so a prompt
/// configured with color codes can be matched against rendered text.
fn strip_csi(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for f in chars.by_ref() {
                if ('\x40'..='\x7e').contains(&f) {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
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
            break_flag: None,
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
                std::thread::sleep(esc_pause());
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

    /// Resize the emulated screen and the PTY (delivers SIGWINCH to the
    /// child) together.
    ///
    /// The emulator resizes first, under the state lock, so any repaint the
    /// child performs in response to SIGWINCH is parsed at the new size —
    /// the same order a real terminal uses (grid first, then signal).
    pub fn resize(&self, rows: u16, cols: u16) {
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.parser.screen_mut().set_size(rows, cols);
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("resize pty");
        state.generation += 1;
        drop(state);
        self.shared.cond.notify_all();
    }

    /// Trigger the fixture's external break signal (requires the builder's
    /// `break_signal()`) and wait for the fixture to acknowledge that
    /// `read_line` returned `Signal::ExternalBreak`.
    pub fn trigger_break(&self) {
        let path = self
            .break_flag
            .as_ref()
            .expect("break_signal() not enabled on this TestTerm");
        let ack = format!("{}.ack", path.display());
        let _ = std::fs::remove_file(&ack);
        std::fs::write(path, b"1").expect("write break flag");
        let deadline = Instant::now() + base_timeout();
        while !std::path::Path::new(&ack).exists() {
            assert!(
                Instant::now() < deadline,
                "fixture never acknowledged the break signal"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = std::fs::remove_file(&ack);
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
    #[allow(dead_code)]
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
                return Ok(());
            }
            // Want-vs-got with the first differing row marked.
            let mut msg = String::from("contents differ\n  want | got\n");
            for (i, (w, g)) in want.iter().zip(got.iter()).enumerate() {
                let mark = if w == g { ' ' } else { '>' };
                msg.push_str(&format!("{mark} {i:>2} {w:?} | {g:?}\n"));
            }
            Err(msg)
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

    /// Assert the text of the row the cursor is on (right-trimmed), wherever
    /// that row is — the go-to assertion for "the buffer now reads X" when
    /// the prompt's row number isn't the point of the test.
    pub fn expect_cursor_line(&self, expected: &str) {
        let expected = expected.trim_end().to_string();
        self.expect(&format!("cursor row to read {expected:?}"), move |screen| {
            let (row, _) = screen.cursor_position();
            let got = row_text(screen, row);
            if got == expected {
                Ok(())
            } else {
                Err(format!("cursor row reads {got:?}"))
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

    /// Assert the screen does NOT change for the standard (CI-scaled)
    /// window — Neovim's `unchanged`. Any output that alters the visible
    /// screen within the window fails.
    pub fn expect_unchanged(&self) {
        self.expect_unchanged_for(unchanged_window());
    }

    /// `expect_unchanged` with an explicit window.
    pub fn expect_unchanged_for(&self, window: Duration) {
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

    /// Wait until the cursor sits on a fresh, empty prompt row: the row
    /// starts with the prompt, and between the prompt and the cursor there
    /// is nothing but an optional vi mode indicator. A right prompt may
    /// trail at the right edge.
    ///
    /// NOTE: the marker comparison counts chars, not display cells, so this
    /// assumes the prompt itself contains no wide glyphs.
    pub fn expect_fresh_prompt(&self) {
        let marker = self.prompt_marker.clone();
        self.expect("a fresh empty prompt at the cursor", move |screen| {
            let (row, col) = screen.cursor_position();
            let text = row_text(screen, row);
            if !text.starts_with(&marker) {
                return Err(format!("cursor row reads {text:?}"));
            }
            let between: String = (0..col)
                .filter_map(|c| screen.cell(row, c))
                .map(|cell| cell.contents())
                .collect::<String>()
                .chars()
                .skip(marker.chars().count())
                .collect();
            match between.trim() {
                "" | "[i]" | "[n]" => Ok(()),
                other => Err(format!("text before cursor: {other:?}")),
            }
        });
    }

    // ---- lifecycle -------------------------------------------------------

    /// Abort any in-progress buffer with Ctrl-C, then `quit`.
    pub fn quit_after_clear(self) {
        self.send("<C-c>");
        // Sync on the *fresh prompt row*, not the host's "(ctrl-c)" echo: a
        // prior abort earlier in the test would satisfy a contains-check
        // immediately and let :quit race the repaint.
        self.expect_fresh_prompt();
        self.quit();
    }

    /// Gracefully stop the fixture (assumes an empty prompt) and assert it
    /// exits cleanly. In vi mode this only works from insert mode (`:quit`
    /// must be typed as text) — use [`TestTerm::quit_from_vi`] when the
    /// mode is normal or unknown.
    pub fn quit(mut self) {
        self.send(":quit<Enter>");
        self.wait_clean_exit(":quit");
    }

    /// Vi-aware quit: abort any in-progress buffer, re-enter insert mode if
    /// the indicator shows normal mode, then `quit`.
    pub fn quit_from_vi(self) {
        self.send("<C-c>");
        self.expect_fresh_prompt();
        let in_normal_mode = {
            let state = self
                .shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let screen = state.parser.screen();
            let (row, _) = screen.cursor_position();
            row_text(screen, row).contains("[n]")
        };
        if in_normal_mode {
            self.send("i");
            self.expect_cursor_line(&format!("{} [i]", self.prompt_marker));
        }
        self.quit();
    }

    /// Wait for the fixture to exit on its own (e.g. after Ctrl-D) and
    /// assert a clean exit status.
    pub fn expect_eof(mut self) {
        self.wait_clean_exit("eof");
    }

    fn wait_clean_exit(&mut self, why: &str) {
        // Wait for EOF on the pty (reader thread sets `eof` and notifies)
        // rather than poll-sleeping on the child.
        let deadline = Instant::now() + base_timeout();
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while !state.eof {
            let now = Instant::now();
            if now >= deadline {
                let screen = dump_screen(state.parser.screen());
                panic!("fixture did not exit ({why})\n--- screen ---\n{screen}");
            }
            let (next, _) = self
                .shared
                .cond
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(|e| {
                    let (guard, timeout) = e.into_inner();
                    (guard, timeout)
                });
            state = next;
        }
        drop(state);
        let status = self.child.wait().expect("wait for fixture");
        assert!(status.success(), "fixture exited with {status:?}");
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
                    Some(cell) => text.push_str(cell.contents()),
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

/// Build the `e2e_fixture` example (idempotent; cargo no-ops when fresh) and
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
    let fixture = profile_dir.join("examples").join("e2e_fixture");

    FIXTURE_BUILD.call_once(|| {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let mut cmd = std::process::Command::new(cargo);
        cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
        cmd.args(["build", "--example", "e2e_fixture"]);
        if cfg!(feature = "external_printer") {
            cmd.args(["--features", "external_printer"]);
        }
        if profile_dir.file_name().map_or(false, |n| n == "release") {
            cmd.arg("--release");
        }
        let status = cmd.status().expect("run cargo build for e2e_fixture");
        assert!(status.success(), "building the e2e_fixture example failed");
    });

    assert!(
        fixture.exists(),
        "fixture binary missing at {}",
        fixture.display()
    );
    fixture
}
