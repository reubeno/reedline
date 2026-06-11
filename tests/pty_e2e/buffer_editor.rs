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
