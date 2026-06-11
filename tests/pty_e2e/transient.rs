//! Transient prompts: submitted lines are repainted with a shortened prompt
//! while the next line gets the full prompt — a repaint at a *previous*
//! prompt's position, which leans directly on the anchor bookkeeping.

use crate::harness::TestTerm;

#[test]
fn submitted_line_repaints_with_transient_prompt() {
    let term = TestTerm::builder().env("FIX_TRANSIENT", "1").spawn();
    term.expect_cursor(0, 5);
    term.send("hello<Enter>");
    term.expect_screen(
        "t> hello\n\
         GOT: hello\n\
         tst> ",
    );
    term.quit();
}

#[test]
fn transient_prompt_applies_per_submission() {
    let term = TestTerm::builder().env("FIX_TRANSIENT", "1").spawn();
    term.expect_cursor(0, 5);
    term.send("one<Enter>");
    term.expect_line(2, "tst>");
    term.send("two<Enter>");
    term.expect_screen(
        "t> one\n\
         GOT: one\n\
         t> two\n\
         GOT: two\n\
         tst> ",
    );
    term.quit();
}

#[test]
fn ctrl_c_does_not_use_transient_prompt() {
    let term = TestTerm::builder().env("FIX_TRANSIENT", "1").spawn();
    term.expect_cursor(0, 5);
    term.send("nope<C-c>");
    term.expect_fresh_prompt();
    // Aborted lines keep the regular prompt (cleared buffer), no transient.
    term.expect_line(0, "tst>");
    term.quit();
}
