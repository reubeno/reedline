//! External printer: messages from another thread are printed above the
//! prompt while `read_line` is active, and the prompt (with any in-progress
//! input) is repainted intact below them.

use crate::harness::{screen_rows, TestTerm};

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
    // Deterministic final state: the filler above must survive the scroll
    // exactly; a stale anchor clobbers or duplicates rows here.
    term.expect_screen(
        "fill-004\n\
         fill-005\n\
         fill-006\n\
         tst> :ext FROM-THREAD\n\
         FROM-THREAD\n\
         tst> ",
    );
    term.send("ok<Enter>");
    term.expect_contains("GOT: ok");
    term.quit();
}

#[test]
fn wrapped_external_message_keeps_prompt_below_it() {
    let term = TestTerm::builder().size(8, 20).spawn();
    term.expect_cursor(0, 5);
    // A 50-cell message wraps to 3 rows at 20 cols, but the painter's row
    // bookkeeping counts each message as one row (painter.rs admits the
    // one-sided drift check cannot catch this).
    let message = "M".repeat(50);
    term.send(&format!(":ext {message}<Enter>"));
    term.expect_contains(&"M".repeat(20));
    term.expect("all message rows above the prompt", |screen| {
        let rows = screen_rows(screen);
        let total_m: usize = rows
            .iter()
            .map(|r| r.chars().filter(|c| *c == 'M').count())
            .sum();
        // 50 from the message; the typed ":ext MMM..." command echo may
        // have scrolled partially or fully off, so require at least 50.
        if total_m < 50 {
            return Err(format!("only {total_m} M cells visible"));
        }
        let last_m = rows.iter().rposition(|r| r.starts_with('M'));
        let prompt = rows.iter().rposition(|r| r.starts_with("tst>"));
        match (last_m, prompt) {
            (Some(m), Some(p)) if p > m => Ok(()),
            (m, p) => Err(format!("message rows at {m:?}, prompt at {p:?}")),
        }
    });
    term.send("ok<Enter>");
    term.expect_contains("GOT: ok");
    term.quit();
}
