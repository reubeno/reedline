//! Cursor-position (DSR / `CSI 6n`) round-trip budget.
//!
//! These pin the perf property of the prompt-anchor cache: reedline asks the
//! terminal where the cursor is exactly once per `read_line` (on entry) and
//! never on the steady-state repaint path.

use std::time::Duration;

use crate::harness::TestTerm;

#[test]
fn exactly_one_dsr_per_read_line_cycle() {
    let term = TestTerm::spawn();
    term.expect_screen("tst> ");
    let after_first_prompt = term.dsr_count();
    assert_eq!(
        after_first_prompt, 1,
        "read_line initialization should query the cursor exactly once"
    );

    for (i, expected_row) in [(1, 2u16), (2, 4), (3, 6)] {
        term.send(&format!("cycle-{i}<Enter>"));
        term.expect_line(expected_row, "tst>");
        assert_eq!(
            term.dsr_count(),
            after_first_prompt + i,
            "each read_line cycle should add exactly one cursor query"
        );
    }
    term.quit();
}

#[test]
fn typing_and_repainting_issue_no_dsr() {
    let term = TestTerm::spawn();
    term.expect_screen("tst> ");
    let baseline = term.dsr_count();

    // Every keystroke repaints; none of these repaints may query.
    term.send("abcdef<Left><Left>XY<Home>Z");
    term.expect_screen("tst> ZabcdXYef");
    // Once painted, the screen must be fully quiescent: no late repaints,
    // queries, or other terminal traffic.
    term.expect_unchanged(Duration::from_millis(200));
    assert_eq!(
        term.dsr_count(),
        baseline,
        "steady-state repaints must not query the cursor position"
    );
    term.quit_after_clear();
}

#[test]
fn clear_screen_costs_at_most_one_dsr() {
    let term = TestTerm::spawn();
    term.expect_screen("tst> ");
    term.send("x<Enter>");
    term.expect_line(2, "tst>");
    let baseline = term.dsr_count();

    term.send("keep<C-l>");
    term.expect_screen("tst> keep");
    let delta = term.dsr_count() - baseline;
    assert!(
        delta <= 1,
        "Ctrl-L re-anchors at row 0; it needs at most one query (saw {delta})"
    );
    term.quit_after_clear();
}
