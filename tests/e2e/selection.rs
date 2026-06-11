//! Text selection via shift-modified movement (bound to `select: true`
//! moves in the default emacs keybindings).

use crate::harness::TestTerm;

#[test]
fn shift_left_selects_and_backspace_deletes_selection() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("abcdef");
    term.expect_cursor(0, 11);
    // Select "ef" backwards, then delete the selection in one keypress.
    term.send("<S-Left><S-Left><BS>");
    term.expect_screen("tst> abcd");
    term.expect_cursor(0, 9);
    term.quit_after_clear();
}

#[test]
fn typing_replaces_selection() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("hello world<C-S-Left>");
    // "world" is selected; typing replaces it.
    term.send("there");
    term.expect_screen("tst> hello there");
    term.send("<Enter>");
    term.expect_contains("GOT: hello there");
    term.quit();
}

#[test]
fn shift_end_selects_to_end_of_line() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("keep cut<Home>");
    term.send("<S-Right><S-Right><S-Right><S-Right><S-Right>");
    // First five cells ("keep ") selected; replace them.
    term.send("x");
    term.expect_screen("tst> xcut");
    term.quit_after_clear();
}
