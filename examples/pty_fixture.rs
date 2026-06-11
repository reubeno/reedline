//! Deterministic reedline instance used by the PTY end-to-end tests
//! (`tests/pty_e2e.rs`). Not intended for interactive use.
//!
//! Behavior is configured entirely through `FIX_*` environment variables so
//! the test harness can spawn one binary in many configurations:
//!
//! - `FIX_PROMPT`            left prompt text (default `tst> `); may contain
//!   `\n` (two characters) which is expanded to a newline for multi-line
//!   prompts
//! - `FIX_RIGHT_PROMPT`      right prompt text (default: none)
//! - `FIX_HISTORY`           newline-separated history entries, oldest first
//! - `FIX_VI=1`              use vi edit mode instead of emacs
//! - `FIX_MENU=1`            enable a columnar completion menu on Tab with a
//!   fixed completer (`alpha`, `alphabet`, `alphard`, `beta`)
//! - `FIX_HIGHLIGHT=1`       enable `ExampleHighlighter` for the word `test`
//!   (match: green, non-match: red, neutral: white)
//! - `FIX_EDITOR_CMD`        shell snippet run as `sh -c <snippet> fixture-editor <tmpfile>`
//!   when the buffer editor opens (`Ctrl-O`); `$1` is the buffer file
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
//! Ctrl-C prints `(ctrl-c)` and continues; Ctrl-D exits.

use std::borrow::Cow;
use std::io::Write;

use reedline::{
    default_emacs_keybindings, default_vi_insert_keybindings, default_vi_normal_keybindings,
    ColumnarMenu, DefaultCompleter, Emacs, ExampleHighlighter, FileBackedHistory, HistoryItem,
    KeyCode, KeyModifiers, MenuBuilder, Prompt, PromptEditMode, PromptHistorySearch, Reedline,
    ReedlineEvent, ReedlineMenu, Signal, Vi,
};

struct FixturePrompt {
    left: String,
    right: String,
}

impl Prompt for FixturePrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.left)
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.right)
    }

    fn render_prompt_indicator(&self, _prompt_mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed("")
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

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

fn env_flag(name: &str) -> bool {
    env(name).is_some_and(|v| v != "0")
}

fn main() -> std::io::Result<()> {
    let prompt = FixturePrompt {
        left: env("FIX_PROMPT")
            .unwrap_or_else(|| "tst> ".into())
            .replace("\\n", "\n"),
        right: env("FIX_RIGHT_PROMPT").unwrap_or_default(),
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

    if env_flag("FIX_MENU") {
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

    if let Some(snippet) = env("FIX_EDITOR_CMD") {
        let temp_file = std::env::temp_dir().join(format!(
            "reedline-pty-fixture-editor-{}.txt",
            std::process::id()
        ));
        let mut command = std::process::Command::new("sh");
        command.arg("-c").arg(snippet).arg("fixture-editor");
        line_editor = line_editor.with_buffer_editor(command, temp_file);
    }

    line_editor = if env_flag("FIX_VI") {
        line_editor.with_edit_mode(Box::new(Vi::new(
            default_vi_insert_keybindings(),
            default_vi_normal_keybindings(),
        )))
    } else {
        let mut keybindings = default_emacs_keybindings();
        if env_flag("FIX_MENU") {
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
            _ => {}
        }
    }

    Ok(())
}
