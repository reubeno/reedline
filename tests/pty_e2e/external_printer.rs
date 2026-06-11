//! External printer: messages from another thread are printed above the
//! prompt while `read_line` is active, and the prompt (with any in-progress
//! input) is repainted intact below them.

use crate::harness::TestTerm;

#[test]
fn message_prints_above_prompt() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    // The fixture queues the message; it is printed during the next
    // read_line as soon as the engine drains the printer queue.
    term.send(":ext ANNOUNCEMENT<Enter>");
    term.expect_screen(
        "tst> :ext ANNOUNCEMENT\n\
         ANNOUNCEMENT\n\
         tst> ",
    );
    term.quit();
}

#[test]
fn in_progress_input_survives_external_message() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send(":ext LATER<Enter>");
    // Race intentionally: type while the message may still be in flight.
    term.send("typed input");
    term.expect_contains("LATER");
    term.expect_contains("tst> typed input");
    // Editing afterwards stays coherent.
    term.send("<Home>X");
    term.expect_contains("tst> Xtyped input");
    term.quit_after_clear();
}

#[test]
fn external_message_at_bottom_scrolls_cleanly() {
    let term = TestTerm::builder().size(6, 30).spawn();
    term.send(":fill 6<Enter>");
    term.expect_line(5, "tst>");
    term.send(":ext FROM-THREAD<Enter>");
    term.expect_contains("FROM-THREAD");
    // Prompt must end up below the message on the bottom row, still usable.
    term.send("ok<Enter>");
    term.expect_contains("GOT: ok");
    term.quit();
}
