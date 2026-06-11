//! Host-driven suspension via the external break signal (issue #1042):
//! `read_line` returns `Signal::ExternalBreak`, and the *next* `read_line`
//! must re-use the prompt's screen rows for a seamless resume — no
//! duplicated prompt, no lost buffer rendering.

use crate::harness::TestTerm;

#[test]
fn external_break_resumes_seamlessly_with_buffer_intact() {
    let term = TestTerm::builder().break_signal().spawn();
    term.expect_cursor(0, 5);
    term.send("draft");
    term.expect_screen("tst> draft");

    // Suspend and resume. The prompt must stay on its row, the buffer must
    // still be rendered, and typing must continue where it left off.
    term.trigger_break();
    term.send("X");
    term.expect_screen("tst> draftX");
    term.expect_cursor(0, 11);

    term.send("<Enter>");
    term.expect_screen(
        "tst> draftX\n\
         GOT: draftX\n\
         tst> ",
    );
    term.quit();
}

#[test]
fn break_on_empty_prompt_reuses_row() {
    let term = TestTerm::builder().break_signal().spawn();
    term.expect_cursor(0, 5);
    term.trigger_break();
    term.send("ok<Enter>");
    // No duplicated or displaced prompt row from the suspension cycle.
    term.expect_screen(
        "tst> ok\n\
         GOT: ok\n\
         tst> ",
    );
    term.quit();
}

#[test]
fn repeated_breaks_do_not_drift_prompt() {
    let term = TestTerm::builder().break_signal().spawn();
    term.expect_cursor(0, 5);
    term.send("steady");
    for _ in 0..3 {
        term.trigger_break();
    }
    term.send("!<Enter>");
    term.expect_screen(
        "tst> steady!\n\
         GOT: steady!\n\
         tst> ",
    );
    term.quit();
}
