//! Deterministic reedline instance used by the PTY end-to-end tests
//! (`tests/pty_e2e/`). Not intended for interactive use.
//!
//! Behavior is configured entirely through `FIX_*` environment variables so
//! the test harness can spawn one binary in many configurations:
//!
//! - `FIX_PROMPT`            left prompt text (default `tst> `); may contain
//!   `\n` (two characters) which is expanded to a newline for multi-line
//!   prompts
//! - `FIX_RIGHT_PROMPT`      right prompt text (default: none)
//! - `FIX_HISTORY`           newline-separated history entries, oldest first
//! - `FIX_VI=1`              use vi edit mode instead of emacs; the prompt
//!   indicator shows `[i] ` / `[n] ` so tests can observe mode changes
//! - `FIX_MENU=1`            columnar completion menu on Tab with a fixed
//!   completer (`alpha`, `alphabet`, `alphard`, `beta`)
//! - `FIX_MENU=ide`          IDE completion menu on Tab with a fixed
//!   completer whose candidates include wide glyphs and descriptions
//! - `FIX_HIGHLIGHT=1`       enable `ExampleHighlighter` for the word `test`
//!   (match: green, non-match: red, neutral: white)
//! - `FIX_HINT=1`            enable `DefaultHinter` (dark gray); needs
//!   `FIX_HISTORY` to have something to hint
//! - `FIX_ABBR`              comma-separated `key=value` abbreviations
//! - `FIX_PARTIAL=1`         enable partial completions
//! - `FIX_EXCLUDE_PREFIX`    history exclusion prefix (e.g. a space)
//! - `FIX_TRANSIENT=1`       transient prompt `t> ` replaces the full prompt
//!   on submitted lines
//! - `FIX_EDITOR_CMD`        shell snippet run as `sh -c <snippet> fixture-editor <tmpfile>`
//!   when the buffer editor opens (`Ctrl-O`); `$1` is the buffer file
//! - `FIX_BREAK_FLAG`        path to a flag file; a watcher thread raises
//!   reedline's external break signal when the file appears
//!
//! REPL protocol on submitted lines:
//!
//! - `:quit`        exit
//! - `:fill N`      print N numbered filler lines (`fill-001` ...)
//! - `:raw TEXT`    print TEXT *without* a trailing newline (leaves the
//!   cursor mid-line, like a host command whose output lacks a final `\n`)
//! - `:ext TEXT`    queue TEXT on the external printer (only with the
//!   `external_printer` feature); it is printed during the next `read_line`
//! - anything else  echo `GOT: <line>`
//!
//! Ctrl-C prints `(ctrl-c)` and continues; Ctrl-D exits. An external break
//! prints `(break)` and continues.

use std::borrow::Cow;
use std::io::Write;

use reedline::{
    default_emacs_keybindings, default_vi_insert_keybindings, default_vi_normal_keybindings,
    ColumnarMenu, Completer, DefaultCompleter, DefaultHinter, Emacs, ExampleHighlighter,
    FileBackedHistory, HistoryItem, IdeMenu, KeyCode, KeyModifiers, MenuBuilder, Prompt,
    PromptEditMode, PromptHistorySearch, PromptViMode, Reedline, ReedlineEvent, ReedlineMenu,
    Signal, Span, Suggestion, Vi,
};

struct FixturePrompt {
    left: String,
    right: String,
    vi_indicators: bool,
}

impl Prompt for FixturePrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.left)
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.right)
    }

    fn render_prompt_indicator(&self, prompt_mode: PromptEditMode) -> Cow<'_, str> {
        if !self.vi_indicators {
            return Cow::Borrowed("");
        }
        match prompt_mode {
            PromptEditMode::Vi(PromptViMode::Insert) => Cow::Borrowed("[i] "),
            PromptEditMode::Vi(PromptViMode::Normal) => Cow::Borrowed("[n] "),
            _ => Cow::Borrowed(""),
        }
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("::: ")
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<'_, str> {
        Cow::Owned(format!("(search:{}) ", history_search.term))
    }
}

/// Fixed suggestions with descriptions and wide glyphs, for IDE-menu tests.
struct DescribingCompleter;

impl Completer for DescribingCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        let word_start = line[..pos].rfind(' ').map(|i| i + 1).unwrap_or(0);
        let word = &line[word_start..pos];
        [
            ("héllo", "greeting"),
            ("héllium", "an élément"),
            ("日本語x", "wide glyphs"),
        ]
        .iter()
        .filter(|(value, _)| value.starts_with(word) && !word.is_empty())
        .map(|(value, description)| Suggestion {
            value: value.to_string(),
            description: Some(description.to_string()),
            span: Span::new(word_start, pos),
            ..Suggestion::default()
        })
        .collect()
    }
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

fn env_flag(name: &str) -> bool {
    env(name).map_or(false, |v| v != "0")
}

fn main() -> std::io::Result<()> {
    let prompt = FixturePrompt {
        left: env("FIX_PROMPT")
            .unwrap_or_else(|| "tst> ".into())
            .replace("\\n", "\n"),
        right: env("FIX_RIGHT_PROMPT").unwrap_or_default(),
        vi_indicators: env_flag("FIX_VI"),
    };

    let mut line_editor = Reedline::create();

    if let Some(entries) = env("FIX_HISTORY") {
        // In-memory only: capacity-bounded, no file behind it.
        let mut history = FileBackedHistory::new(100).expect("valid capacity");
        for entry in entries.split('\n').filter(|e| !e.is_empty()) {
            use reedline::History;
            history
                .save(HistoryItem::from_command_line(entry))
                .expect("seed history");
        }
        line_editor = line_editor.with_history(Box::new(history));
    }

    if env_flag("FIX_HIGHLIGHT") {
        let mut highlighter = ExampleHighlighter::new(vec!["test".into()]);
        highlighter.change_colors(
            nu_ansi_term::Color::Green,
            nu_ansi_term::Color::Red,
            nu_ansi_term::Color::White,
        );
        line_editor = line_editor.with_highlighter(Box::new(highlighter));
    }

    if env_flag("FIX_HINT") {
        line_editor = line_editor.with_hinter(Box::new(DefaultHinter::default().with_style(
            nu_ansi_term::Style::new().fg(
                // Idx(8) on the emulated screen.
                nu_ansi_term::Color::DarkGray,
            ),
        )));
    }

    if let Some(abbrs) = env("FIX_ABBR") {
        let map: std::collections::HashMap<String, String> = abbrs
            .split(',')
            .filter_map(|pair| pair.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        line_editor = line_editor.with_abbreviations(map);
    }

    if let Some(prefix) = std::env::var("FIX_EXCLUDE_PREFIX")
        .ok()
        .filter(|p| !p.is_empty())
    {
        line_editor = line_editor.with_history_exclusion_prefix(Some(prefix));
    }

    if env_flag("FIX_PARTIAL") {
        line_editor = line_editor.with_partial_completions(true);
    }

    if env_flag("FIX_TRANSIENT") {
        line_editor = line_editor.with_transient_prompt(Box::new(FixturePrompt {
            left: "t> ".into(),
            right: String::new(),
            vi_indicators: false,
        }));
    }

    let menu_mode = env("FIX_MENU");
    match menu_mode.as_deref() {
        Some("ide") => {
            let menu = IdeMenu::default().with_name("completion_menu");
            line_editor = line_editor
                .with_completer(Box::new(DescribingCompleter))
                .with_menu(ReedlineMenu::EngineCompleter(Box::new(menu)));
        }
        Some(_) => {
            let completer = DefaultCompleter::new_with_wordlen(
                vec![
                    "alpha".into(),
                    "alphabet".into(),
                    "alphard".into(),
                    "beta".into(),
                ],
                1,
            );
            let menu = ColumnarMenu::default().with_name("completion_menu");
            line_editor = line_editor
                .with_completer(Box::new(completer))
                .with_menu(ReedlineMenu::EngineCompleter(Box::new(menu)));
        }
        None => {}
    }

    if let Some(snippet) = env("FIX_EDITOR_CMD") {
        let temp_file = std::env::temp_dir().join(format!(
            "reedline-pty-fixture-editor-{}.txt",
            std::process::id()
        ));
        let mut command = std::process::Command::new("sh");
        command.arg("-c").arg(snippet).arg("fixture-editor");
        line_editor = line_editor.with_buffer_editor(command, temp_file);
    }

    let break_flag = env("FIX_BREAK_FLAG");
    if let Some(flag_path) = break_flag.clone() {
        let break_signal = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        line_editor = line_editor.with_break_signal(break_signal.clone());
        std::thread::spawn(move || loop {
            if std::path::Path::new(&flag_path).exists() {
                let _ = std::fs::remove_file(&flag_path);
                break_signal.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        });
    }

    line_editor = if env_flag("FIX_VI") {
        line_editor.with_edit_mode(Box::new(Vi::new(
            default_vi_insert_keybindings(),
            default_vi_normal_keybindings(),
        )))
    } else {
        let mut keybindings = default_emacs_keybindings();
        if menu_mode.is_some() {
            keybindings.add_binding(
                KeyModifiers::NONE,
                KeyCode::Tab,
                ReedlineEvent::UntilFound(vec![
                    ReedlineEvent::Menu("completion_menu".to_string()),
                    ReedlineEvent::MenuNext,
                ]),
            );
        }
        line_editor.with_edit_mode(Box::new(Emacs::new(keybindings)))
    };

    #[cfg(feature = "external_printer")]
    let external_printer = {
        let printer = reedline::ExternalPrinter::default();
        line_editor = line_editor.with_external_printer(printer.clone());
        printer
    };

    loop {
        match line_editor.read_line(&prompt)? {
            Signal::Success(line) => {
                if line == ":quit" {
                    break;
                } else if let Some(n) = line.strip_prefix(":fill ") {
                    let n: usize = n.trim().parse().unwrap_or(0);
                    for i in 1..=n {
                        println!("fill-{i:03}");
                    }
                } else if let Some(text) = line.strip_prefix(":raw ") {
                    print!("{text}");
                    std::io::stdout().flush()?;
                } else if line == ":dsr" {
                    // Emulates a host command that queries the cursor
                    // position itself (`printf "\e[6n"`, issue #860): the
                    // terminal's reply arrives on stdin outside read_line.
                    print!("\x1b[6n");
                    std::io::stdout().flush()?;
                } else if let Some(_rest) = line.strip_prefix(":extn ") {
                    #[cfg(feature = "external_printer")]
                    if let Some((n, text)) = _rest.split_once(' ') {
                        let n: usize = n.parse().unwrap_or(0);
                        for i in 1..=n {
                            external_printer
                                .print(format!("{text}-{i}"))
                                .expect("external printer queue");
                        }
                    }
                } else if let Some(_text) = line.strip_prefix(":ext ") {
                    #[cfg(feature = "external_printer")]
                    external_printer
                        .print(_text.to_string())
                        .expect("external printer queue");
                } else {
                    println!("GOT: {line}");
                }
            }
            Signal::CtrlC => {
                println!("(ctrl-c)");
            }
            Signal::CtrlD => break,
            Signal::ExternalBreak(_) => {
                // The break is a *suspension*: printing here would be
                // painted over when the next read_line re-uses the prompt
                // rows. Acknowledge out-of-band instead so tests can
                // confirm the break fired.
                if let Some(flag) = &break_flag {
                    let _ = std::fs::write(format!("{flag}.ack"), b"1");
                }
            }
            _ => {}
        }
    }

    Ok(())
}
