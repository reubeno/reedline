//! Right-hand-side prompt rendering.

use crate::harness::{screen_rows, TestTerm};

#[test]
fn right_prompt_renders_at_right_edge() {
    let term = TestTerm::builder().size(6, 20).right_prompt("RP").spawn();
    // 20 cols: "tst> " (5) + 13 spaces + "RP" (2).
    term.expect_screen(&format!("tst> {}RP", " ".repeat(13)));
    term.expect_cursor(0, 5);
    term.quit();
}

#[test]
fn right_prompt_survives_typing() {
    let term = TestTerm::builder().size(6, 30).right_prompt("RP").spawn();
    term.expect_contains("RP");
    term.send("abc");
    // 30 cols: "tst> abc" (8) + 20 spaces + "RP" (2).
    term.expect_line(0, &format!("tst> abc{}RP", " ".repeat(20)));
    term.expect_cursor(0, 8);
    term.quit_after_clear();
}

#[test]
fn right_prompt_yields_when_input_reaches_it() {
    let term = TestTerm::builder().size(6, 20).right_prompt("RP").spawn();
    term.expect_contains("RP");
    // The right prompt stays while `input_width <= rp.start_col` (18 here);
    // 14 input cols push past that, so it must disappear rather than
    // collide with the input.
    let input = "i".repeat(14);
    let expected = format!("tst> {input}");
    term.send(&input);
    term.expect("right prompt to yield to long input", move |screen| {
        let row = &screen_rows(screen)[0];
        if row.ends_with("RP") {
            Err(format!("right prompt still present: {row:?}"))
        } else if *row == expected {
            Ok(())
        } else {
            Err(format!("unexpected row 0: {row:?}"))
        }
    });
    term.quit_after_clear();
}

#[test]
fn right_prompt_repaints_after_resize() {
    let term = TestTerm::builder().size(6, 40).right_prompt("RP").spawn();
    term.expect_contains("RP");
    term.resize(6, 25);
    term.expect_line(0, &format!("tst> {}RP", " ".repeat(18)));
    term.quit();
}
