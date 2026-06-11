//! Editing *within* a multi-line buffer: cursor movement between lines,
//! mid-buffer edits, line joins. Repaints here touch multiple rows at once.

use crate::harness::TestTerm;

#[test]
fn up_arrow_moves_into_previous_line_not_history() {
    let term = TestTerm::builder().history(&["decoy entry"]).spawn();
    term.expect_cursor(0, 5);
    term.send("first<A-Enter>second");
    term.expect_cursor(1, 10);
    // With the cursor on a lower line, Up must move within the buffer,
    // not recall history.
    term.send("<Up>");
    term.expect("cursor on the first buffer line", |screen| {
        let (row, _) = screen.cursor_position();
        if row == 0 {
            Ok(())
        } else {
            Err(format!("cursor on row {row}"))
        }
    });
    term.send("X");
    term.expect_line(0, "tst> firstX");
    term.expect_line(1, "::: second");
    term.quit_after_clear();
}

#[test]
fn down_arrow_returns_to_lower_line() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("aaa<A-Enter>bbb<Up><Down>!");
    term.expect_screen(
        "tst> aaa\n\
         ::: bbb!",
    );
    term.quit_after_clear();
}

#[test]
fn home_end_apply_to_current_line_only() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("top line<A-Enter>bottom");
    term.send("<Home>");
    term.expect_cursor(1, 4); // start of "bottom", after "::: "
    term.send(">");
    term.expect_line(1, "::: >bottom");
    term.send("<End>!");
    term.expect_line(1, "::: >bottom!");
    // The other line is untouched by the repaints.
    term.expect_line(0, "tst> top line");
    term.quit_after_clear();
}

#[test]
fn editing_middle_line_of_three() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("one<A-Enter>two<A-Enter>three");
    term.expect_screen(
        "tst> one\n\
         ::: two\n\
         ::: three",
    );
    term.send("<Up><End>-edit");
    term.expect_screen(
        "tst> one\n\
         ::: two-edit\n\
         ::: three",
    );
    // The whole buffer survives submission.
    term.send("<Enter>");
    term.expect_contains("GOT: one");
    term.expect_contains("two-edit");
    term.quit();
}

#[test]
fn backspace_at_line_start_joins_lines() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("abc<A-Enter>def");
    term.expect_screen(
        "tst> abc\n\
         ::: def",
    );
    // Move to the start of "def" and delete the newline: the lines join
    // and the second row must be cleared by the shrinking repaint.
    term.send("<Home><BS>");
    term.expect_screen("tst> abcdef");
    term.expect_cursor(0, 8);
    term.send("<Enter>");
    term.expect_contains("GOT: abcdef");
    term.quit();
}

#[test]
fn deleting_last_line_clears_its_row() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("keep<A-Enter>gone");
    term.expect_line(1, "::: gone");
    // Delete the second line's content; the continuation indicator remains
    // until the newline itself is removed. (Note: Ctrl-U cuts from the
    // start of the *buffer*, not the line, so plain backspaces are used.)
    term.send("<BS><BS><BS><BS>");
    term.expect_screen(
        "tst> keep\n\
         :::",
    );
    term.send("<BS>");
    term.expect_screen("tst> keep");
    term.expect_cursor(0, 9);
    term.quit_after_clear();
}
