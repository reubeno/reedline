//! Ctrl-C abort and Ctrl-L clear-screen behavior (UX checklist "Clearing").

use crate::harness::TestTerm;

#[test]
fn ctrl_c_aborts_to_fresh_prompt_below() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("abandon me<C-c>");
    // Reedline clears the aborted input from the screen (EditCommand::Clear
    // runs on CtrlC), so the old prompt line ends up empty.
    term.expect_screen(
        "tst> \n\
         (ctrl-c)\n\
         tst> ",
    );
    term.expect_cursor(2, 5);
    term.quit();
}

#[test]
fn ctrl_l_moves_prompt_to_top_and_preserves_buffer() {
    let term = TestTerm::spawn();
    // Put the prompt on a non-zero row first so the move is observable.
    term.send("one<Enter>");
    term.expect_screen(
        "tst> one\n\
         GOT: one\n\
         tst> ",
    );

    term.send("keep me<C-l>");
    term.expect_screen("tst> keep me");
    term.expect_cursor(0, 12);

    // The preserved buffer still submits normally.
    term.send("<Enter>");
    term.expect_screen(
        "tst> keep me\n\
         GOT: keep me\n\
         tst> ",
    );
    term.quit();
}

#[test]
fn ctrl_l_with_multiline_buffer_repaints_whole_buffer() {
    let term = TestTerm::spawn();
    term.send("one<Enter>");
    term.expect_line(2, "tst>");
    term.send("aa<A-Enter>bb<A-Enter>cc<C-l>");
    term.expect_screen(
        "tst> aa\n\
         ::: bb\n\
         ::: cc",
    );
    term.expect_cursor(2, 6);
    term.send("<Enter>");
    term.expect_contains("GOT: aa");
    term.quit();
}

#[test]
fn ctrl_l_when_prompt_already_at_top_is_idempotent() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("abc<C-l>");
    term.expect_screen("tst> abc");
    term.expect_cursor(0, 8);
    // A no-op clear must not scroll, duplicate, or repaint endlessly.
    term.expect_unchanged();
    term.quit_after_clear();
}

#[test]
fn ctrl_l_then_further_editing_stays_coherent() {
    let term = TestTerm::spawn();
    term.send("one<Enter>");
    term.expect_line(2, "tst>");
    term.send("abc<C-l>");
    term.expect_screen("tst> abc");
    term.send("<Left>X<End>!");
    term.expect_screen("tst> abXc!");
    term.quit_after_clear();
}
