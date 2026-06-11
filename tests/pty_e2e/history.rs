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
