//! History recall and reverse search (UX checklist "History").

use crate::harness::TestTerm;

fn with_history() -> TestTerm {
    TestTerm::builder()
        .history(&["first command", "second command"])
        .spawn()
}

#[test]
fn up_arrow_recalls_most_recent_entry_first() {
    let term = with_history();
    term.expect_cursor(0, 5);
    term.send("<Up>");
    term.expect_screen("tst> second command");
    term.send("<Up>");
    term.expect_screen("tst> first command");
    term.send("<Down>");
    term.expect_screen("tst> second command");
    term.send("<Enter>");
    term.expect_contains("GOT: second command");
    term.quit();
}

#[test]
fn prefix_typed_then_up_finds_matching_entry() {
    let term = with_history();
    term.expect_cursor(0, 5);
    term.send("fir<Up>");
    term.expect_screen("tst> first command");
    term.send("<Enter>");
    term.expect_contains("GOT: first command");
    term.quit();
}

#[test]
fn ctrl_r_reverse_search_finds_and_aborts() {
    let term = with_history();
    term.expect_cursor(0, 5);
    term.send("<C-r>");
    term.expect_contains("(search:)");
    term.send("fir");
    term.expect_contains("(search:fir)");
    term.expect_contains("first command");

    // Ctrl-C aborts: reedline exits the read_line with Signal::CtrlC. The
    // search line remains on screen as history and a fresh prompt starts
    // below the host's acknowledgement.
    term.send("<C-c>");
    term.expect_screen(
        "tst> (search:fir) first command\n\
         (ctrl-c)\n\
         tst> ",
    );
    term.expect_cursor(2, 5);
    term.quit();
}

#[test]
fn ctrl_r_cycles_through_multiple_matches() {
    let term = TestTerm::builder()
        .history(&["first match", "second match"])
        .spawn();
    term.expect_cursor(0, 5);
    term.send("<C-r>match");
    // Most recent match first; Ctrl-R again walks to the older one
    // (UX checklist: "Can you find more hits by pressing Ctrl-r?").
    term.expect_contains("(search:match) second match");
    term.send("<C-r>");
    term.expect_contains("(search:match) first match");
    term.send("<C-c>");
    term.expect_fresh_prompt();
    term.quit();
}

#[test]
fn ctrl_r_enter_accepts_match_into_buffer() {
    let term = with_history();
    term.expect_cursor(0, 5);
    term.send("<C-r>fir<Enter>");
    // Enter accepts the match into the buffer and leaves search; it does
    // not submit. A second Enter submits.
    term.expect_screen("tst> first command");
    term.expect_cursor(0, 18);
    term.send("<Enter>");
    term.expect_contains("GOT: first command");
    term.quit();
}

#[test]
fn resubmitting_recalled_entry_does_not_duplicate_history() {
    let term = TestTerm::builder().history(&["older", "recent"]).spawn();
    term.expect_cursor(0, 5);
    term.send("<Up><Enter>"); // recall "recent" and run it again
    term.expect_contains("GOT: recent");
    // UX checklist: the re-run must not be duplicated in history. With no
    // duplicate, two <Up> presses reach "older"; with one, they would land
    // on "recent" twice.
    term.send("<Up>");
    term.expect_cursor_line("tst> recent");
    term.send("<Up>");
    term.expect_cursor_line("tst> older");
    term.send("<C-c>");
    term.expect_fresh_prompt();
    term.quit();
}

#[test]
fn multiline_history_entry_recalls_and_searches() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("top<A-Enter>bottom<Enter>");
    term.expect_contains("GOT: top");
    // Recall renders the entry across two rows again.
    term.send("<Up>");
    term.expect_contains("tst> top");
    term.expect_contains("::: bottom");
    term.send("<C-c>");
    term.expect_fresh_prompt();
    // Reverse search against a multi-line entry must render sanely.
    term.send("<C-r>bot");
    term.expect_contains("(search:bot)");
    term.send("<C-c>");
    term.expect_fresh_prompt();
    term.quit();
}

#[test]
fn exclusion_prefix_keeps_entry_out_of_history() {
    let term = TestTerm::builder().env("FIX_EXCLUDE_PREFIX", " ").spawn();
    term.expect_cursor(0, 5);
    term.send(" secret<Enter>");
    term.expect_contains("GOT:  secret");
    term.send("visible<Enter>");
    term.expect_contains("GOT: visible");
    // Two recalls: the space-prefixed entry must never reappear.
    term.send("<Up>");
    term.expect_cursor_line("tst> visible");
    term.send("<Up>");
    term.expect_unchanged();
    term.send("<C-c>");
    term.expect_fresh_prompt();
    term.quit();
}

#[test]
fn recalled_entry_renders_then_edits_cleanly() {
    let term = with_history();
    term.expect_cursor(0, 5);
    term.send("<Up>");
    term.expect_screen("tst> second command");
    term.send("<End> edited");
    term.expect_screen("tst> second command edited");
    term.send("<Enter>");
    term.expect_contains("GOT: second command edited");
    term.quit();
}
