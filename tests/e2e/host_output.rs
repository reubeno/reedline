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
    // The partial host line above must remain untouched by repaints —
    // enforced as a real negative: nothing may change once painted.
    term.expect_line(1, "stub");
    term.expect_unchanged(crate::harness::unchanged_window());
    term.quit_after_clear();
}

#[test]
fn host_output_ending_exactly_at_last_column() {
    let term = TestTerm::builder().size(6, 20).spawn();
    term.expect_cursor(0, 5);
    // ":raw " + 14 chars = a 20-cell output row: the cursor ends pending at
    // the last column. The next prompt must start on a fresh row without
    // clobbering the output (this is the edge the `position.1 + 1 < row`
    // tolerance in the drift check exists for).
    term.send(":raw ABCDEFGHIJKLMN<Enter>");
    term.expect_contains("ABCDEFGHIJKLMN");
    term.expect("prompt below the full-width output", |screen| {
        let rows = crate::harness::screen_rows(screen);
        let out = rows.iter().position(|r| r.ends_with("ABCDEFGHIJKLMN"));
        let prompt = rows.iter().rposition(|r| r == "tst>");
        match (out, prompt) {
            (Some(o), Some(p)) if p > o => Ok(()),
            (o, p) => Err(format!("output at {o:?}, prompt at {p:?}")),
        }
    });
    term.send("ok<Enter>");
    term.expect_contains("GOT: ok");
    term.quit();
}
