//! Degenerate terminal sizes: the painter must stay panic-free and usable.

use crate::harness::TestTerm;

#[test]
fn two_row_terminal_is_usable() {
    let term = TestTerm::builder().size(2, 30).spawn();
    term.expect_line(0, "tst>");
    term.send("hi<Enter>");
    // Output scrolls through a 2-row window; the session must keep working.
    term.expect_contains("tst>");
    term.send("again<Enter>");
    term.expect_contains("GOT: again");
    term.quit();
}

#[test]
fn very_narrow_terminal_survives_input() {
    let term = TestTerm::builder().size(10, 8).spawn();
    term.expect_contains("tst>");
    term.send("abcdefgh<Enter>"); // wraps repeatedly at 8 cols
    term.expect_contains("GOT:");
    term.quit();
}

// NOTE: no 1-row test: the vt100 crate itself panics (subtract overflow in
// grid scrolling) on single-row screens, so the harness cannot observe one.

#[test]
fn shrink_to_tiny_then_restore() {
    let term = TestTerm::builder().size(12, 40).spawn();
    term.send("resilient");
    term.expect_screen("tst> resilient");
    term.resize(2, 10);
    term.expect_contains("res"); // some part of the buffer is visible
    term.resize(12, 40);
    term.expect_contains("tst> resilient");
    term.send("<Enter>");
    term.expect_contains("GOT: resilient");
    term.quit();
}
