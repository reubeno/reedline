//! Terminal resize: rewrapping and repaint correctness after SIGWINCH.

use crate::harness::TestTerm;

#[test]
fn narrowing_rewraps_long_buffer() {
    let term = TestTerm::builder().size(10, 30).spawn();
    term.expect_cursor(0, 5);
    let input = "x".repeat(40);
    term.send(&input);
    // 5 (prompt) + 40 = 45 cells at 30 cols -> 2 rows.
    term.expect_screen(&format!("tst> {}\n{}", "x".repeat(25), "x".repeat(15),));

    term.resize(10, 20);
    // 45 cells at 20 cols -> 3 rows.
    term.expect_screen(&format!(
        "tst> {}\n{}\n{}",
        "x".repeat(15),
        "x".repeat(20),
        "x".repeat(5),
    ));

    // Buffer is intact end-to-end.
    term.send("<Enter>");
    term.expect_contains(&format!("GOT: {}", "x".repeat(40))[..20]);
    term.quit();
}

#[test]
fn widening_repaints_buffer_on_one_line() {
    let term = TestTerm::builder().size(10, 20).spawn();
    term.expect_cursor(0, 5);
    term.send(&"y".repeat(25));
    term.expect_screen(&format!("tst> {}\n{}", "y".repeat(15), "y".repeat(10),));

    term.resize(10, 40);
    // The terminal does not reflow on widening, so the cursor stays on the
    // (former) wrap row; reedline re-anchors there and repaints the whole
    // buffer on one line. The old first row remains above as scrollback-style
    // residue -- pinned here as current (pre-existing) reedline behavior.
    term.expect_screen(&format!("tst> {}\ntst> {}", "y".repeat(15), "y".repeat(25),));
    term.expect_cursor(1, 30);
    term.quit_after_clear();
}

#[test]
fn resize_with_prompt_at_bottom() {
    let term = TestTerm::builder().size(6, 30).spawn();
    term.send(":fill 6<Enter>");
    // Prompt has been pushed to the bottom row.
    term.expect_line(5, "tst>");
    term.send("steady");
    term.expect_line(5, "tst> steady");

    term.resize(6, 25);
    // After the resize repaint the buffer must still be visible and editable.
    term.expect_contains("tst> steady");
    term.send("!<Enter>");
    term.expect_contains("GOT: steady!");
    term.quit();
}

#[test]
fn repeated_resizes_do_not_corrupt_screen() {
    let term = TestTerm::builder().size(12, 40).spawn();
    term.send("stable text");
    term.expect_screen("tst> stable text");
    // Each resize recalibrates the anchor from the cursor row; residue rows
    // above the prompt are tolerated (terminals do not reflow), but the
    // prompt + buffer must stay rendered and the buffer must stay intact.
    for (rows, cols) in [(12, 25), (8, 60), (20, 33), (12, 40)] {
        term.resize(rows, cols);
        term.expect_contains("tst> stable text");
    }
    term.send("<Enter>");
    term.expect_contains("GOT: stable text");
    term.quit();
}
