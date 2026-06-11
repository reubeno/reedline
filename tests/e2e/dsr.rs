//! Cursor-position (DSR / `CSI 6n`) round-trip budget.
//!
//! These pin the perf property of the prompt-anchor cache: reedline asks the
//! terminal where the cursor is exactly once per `read_line` (on entry) and
//! never on the steady-state repaint path.


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
    term.expect_unchanged();
    assert_eq!(
        term.dsr_count(),
        baseline,
        "steady-state repaints must not query the cursor position"
    );
    term.quit_after_clear();
}

#[test]
fn clear_screen_costs_exactly_one_dsr() {
    let term = TestTerm::spawn();
    term.expect_screen("tst> ");
    term.send("x<Enter>");
    term.expect_line(2, "tst>");
    let baseline = term.dsr_count();

    term.send("keep<C-l>");
    term.expect_screen("tst> keep");
    let delta = term.dsr_count() - baseline;
    // clear_screen re-measures via initialize_prompt_position. The cursor is
    // at (0,0) by construction there, so this could legitimately become 0 if
    // that query is ever optimized away — update this pin when it does.
    assert_eq!(
        delta, 1,
        "Ctrl-L re-anchors at row 0 with exactly one query (saw {delta})"
    );
    term.quit_after_clear();
}

#[test]
fn menu_and_history_search_issue_no_dsr() {
    let term = TestTerm::builder()
        .completion_menu()
        .history(&["alpha one"])
        .spawn();
    term.expect_screen("tst> ");
    let base = term.dsr_count();

    // Menu open, navigate, close: all plain repaints.
    term.send("al<Tab>");
    term.expect_contains("alphabet");
    term.send("<Tab><Esc>");
    term.expect_line(0, "tst> al");
    assert_eq!(term.dsr_count(), base, "menu interaction must not query");

    // History search repaints.
    term.send("<C-r>al");
    term.expect_contains("(search:al)");
    assert_eq!(term.dsr_count(), base, "history search must not query");

    // Aborting costs the next read_line's single init query. Note: Ctrl-C
    // from history search restores the pre-search buffer ("al") into the
    // next read_line rather than clearing it.
    term.send("<C-c>");
    term.expect_line(2, "tst> al");
    assert_eq!(term.dsr_count(), base + 1);
    term.quit_after_clear();
}

#[test]
fn history_recall_issues_no_dsr() {
    let term = TestTerm::builder().history(&["first", "second"]).spawn();
    term.expect_screen("tst> ");
    let base = term.dsr_count();
    term.send("<Up><Up><Down>");
    term.expect_screen("tst> second");
    assert_eq!(term.dsr_count(), base, "history recall must not query");
    term.quit_after_clear();
}

#[test]
fn buffer_editor_costs_exactly_one_dsr() {
    let term = TestTerm::builder()
        .editor_cmd("printf 'edited' > \"$1\"")
        .spawn();
    term.expect_screen("tst> ");
    let base = term.dsr_count();
    term.send("d<C-o>");
    term.expect_screen("tst> edited");
    assert_eq!(
        term.dsr_count(),
        base + 1,
        "returning from the editor re-measures the anchor exactly once"
    );
    term.quit_after_clear();
}

#[test]
fn resize_costs_at_most_two_dsr() {
    let term = TestTerm::builder().size(12, 40).spawn();
    term.expect_screen("tst> ");
    term.send("abc");
    term.expect_screen("tst> abc");
    let base = term.dsr_count();
    term.resize(16, 60);
    term.expect_contains("tst> abc");
    let delta = term.dsr_count() - base;
    // handle_resize measures once; the next repaint re-verifies the Stale
    // anchor with one more query.
    assert!(
        delta <= 2,
        "resize should cost at most 2 queries, saw {delta}"
    );
    term.quit_after_clear();
}

#[cfg(feature = "external_printer")]
#[test]
fn external_message_costs_at_most_two_dsr() {
    let term = TestTerm::spawn();
    term.expect_screen("tst> ");
    let base = term.dsr_count();
    // One query for the post-submit read_line init; one to re-verify
    // after the untracked message rows. Sync on the *standalone* message
    // row (row 1) — the typed ":ext ping" echo on row 0 also contains
    // "ping" and would satisfy a contains-check before the message prints.
    term.send(":ext ping<Enter>");
    term.expect_line(1, "ping");
    term.send("x");
    term.expect_cursor_line("tst> x");
    let delta = term.dsr_count() - base;
    assert!(
        delta <= 2,
        "external message should cost at most 2 queries, saw {delta}"
    );
    term.quit_after_clear();
}
