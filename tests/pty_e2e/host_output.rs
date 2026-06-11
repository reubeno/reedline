//! Host output between read_line calls, including output that leaves the
//! cursor mid-line (no trailing newline): reedline must start the next
//! prompt on a fresh row without clobbering the partial line.

use crate::harness::TestTerm;

#[test]
fn partial_host_output_line_is_preserved() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send(":raw partial<Enter>");
    term.expect_screen(
        "tst> :raw partial\n\
         partial\n\
         tst> ",
    );
    term.expect_cursor(2, 5);
    term.quit();
}

#[test]
fn full_host_output_lines_stack_normally() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send(":fill 3<Enter>");
    term.expect_screen(
        "tst> :fill 3\n\
         fill-001\n\
         fill-002\n\
         fill-003\n\
         tst> ",
    );
    term.quit();
}

#[test]
fn prompt_after_partial_output_paints_and_edits_cleanly() {
    let term = TestTerm::spawn();
    term.send(":raw stub<Enter>");
    term.expect_line(2, "tst>");
    term.send("abc<Left>X");
    term.expect_line(2, "tst> abXc");
    // The partial host line above must remain untouched by repaints.
    term.expect_line(1, "stub");
    term.quit_after_clear();
}
