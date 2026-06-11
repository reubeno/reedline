//! External buffer editor (`Ctrl-O`): the child process owns the tty while
//! it runs, so reedline must re-anchor the prompt afterwards.

use crate::harness::TestTerm;

#[test]
fn editor_replaces_buffer() {
    let term = TestTerm::builder()
        .editor_cmd("printf 'from-editor' > \"$1\"")
        .spawn();
    term.expect_cursor(0, 5);
    term.send("draft<C-o>");
    term.expect_screen("tst> from-editor");
    term.send("<Enter>");
    term.expect_contains("GOT: from-editor");
    term.quit();
}

#[test]
fn editor_that_prints_to_tty_does_not_corrupt_repaint() {
    let term = TestTerm::builder()
        .editor_cmd("echo EDITOR-NOISE; echo MORE-NOISE; printf 'clean' > \"$1\"")
        .spawn();
    term.expect_cursor(0, 5);
    term.send("draft<C-o>");
    // The editor's stray output scrolled the screen; the prompt must
    // re-anchor below it instead of overwriting at the stale row.
    term.expect_contains("EDITOR-NOISE");
    term.expect_contains("MORE-NOISE");
    term.expect_contains("tst> clean");
    term.expect("prompt to sit below the editor noise", |screen| {
        let rows = crate::harness::screen_rows(screen);
        // Raw-mode LF preserves the column, so noise lines may be indented.
        let noise = rows.iter().position(|r| r.trim() == "MORE-NOISE");
        let prompt = rows.iter().position(|r| r.starts_with("tst> clean"));
        match (noise, prompt) {
            (Some(n), Some(p)) if p > n => Ok(()),
            (n, p) => Err(format!("noise at {n:?}, prompt at {p:?}")),
        }
    });
    term.send("<Enter>");
    term.expect_contains("GOT: clean");
    term.quit();
}

#[test]
fn editor_leaving_stale_sgr_does_not_tint_prompt() {
    // Regression guard for #992: a host process can leave SGR attributes
    // (color, italics) open; the repaint must reset them before painting.
    let term = TestTerm::builder()
        .editor_cmd("printf '\\033[41m\\033[3m'; printf 'x' > \"$1\"")
        .spawn();
    term.expect_cursor(0, 5);
    term.send("d<C-o>");
    term.expect_screen("tst> x");
    term.expect("prompt cells unstyled", |screen| {
        for col in 0..6u16 {
            let cell = screen.cell(0, col).ok_or("missing cell")?;
            if cell.bgcolor() != vt100::Color::Default {
                return Err(format!("cell (0, {col}) has bg {:?}", cell.bgcolor()));
            }
            if cell.italic() {
                return Err(format!("cell (0, {col}) is italic"));
            }
        }
        Ok(())
    });
    term.quit_after_clear();
}

#[test]
fn editor_with_prompt_at_bottom_row() {
    let term = TestTerm::builder()
        .size(6, 30)
        .editor_cmd("echo NOISE; printf 'done' > \"$1\"")
        .spawn();
    term.send(":fill 6<Enter>");
    term.expect_line(5, "tst>");
    // Editor noise at the bottom row forces a scroll during re-anchor.
    term.send("x<C-o>");
    term.expect_contains("tst> done");
    term.send("<Enter>");
    term.expect_contains("GOT: done");
    term.quit();
}

#[test]
fn failing_editor_keeps_original_buffer() {
    let term = TestTerm::builder().editor_cmd("exit 1").spawn();
    term.expect_cursor(0, 5);
    term.send("draft<C-o>");
    // The editor exited non-zero without touching the file: the buffer
    // written out comes straight back.
    term.expect_screen("tst> draft");
    term.send("<Enter>");
    term.expect_contains("GOT: draft");
    term.quit();
}

#[test]
fn editor_returning_multiline_buffer_paints_continuations() {
    let term = TestTerm::builder()
        .editor_cmd("printf 'l1\\nl2' > \"$1\"")
        .spawn();
    term.expect_cursor(0, 5);
    term.send("x<C-o>");
    term.expect_screen(
        "tst> l1\n\
         ::: l2",
    );
    term.send("<Enter>");
    term.expect_contains("GOT: l1");
    term.quit();
}

#[test]
fn editor_keeping_buffer_unchanged_repaints_faithfully() {
    let term = TestTerm::builder()
        .editor_cmd(":") // no-op: leaves the buffer file as written
        .spawn();
    term.expect_cursor(0, 5);
    term.send("unchanged<C-o>");
    term.expect_screen("tst> unchanged");
    term.send("<Enter>");
    term.expect_contains("GOT: unchanged");
    term.quit();
}
