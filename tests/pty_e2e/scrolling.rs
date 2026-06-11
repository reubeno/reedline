//! Scrolling: prompt anchored at the bottom, multi-line buffers that force
//! scrolls, and wide characters wrapping at the bottom row (the class of bug
//! a stale prompt anchor produces: duplicated or overwritten rows).

use crate::harness::TestTerm;

#[test]
fn host_output_scrolls_prompt_to_bottom() {
    let term = TestTerm::builder().size(8, 30).spawn();
    term.expect_cursor(0, 5);
    term.send(":fill 10<Enter>");
    // 1 command echo + 10 filler + new prompt = 12 lines in 8 rows;
    // the visible window is the last 8.
    term.expect_screen(
        "fill-004\n\
         fill-005\n\
         fill-006\n\
         fill-007\n\
         fill-008\n\
         fill-009\n\
         fill-010\n\
         tst> ",
    );
    term.expect_cursor(7, 5);
    term.quit();
}

#[test]
fn growing_multiline_buffer_scrolls_anchor_up() {
    let term = TestTerm::builder().size(8, 30).spawn();
    term.send(":fill 10<Enter>");
    term.expect_line(7, "tst>");

    // Each Alt-Enter adds a continuation line; the prompt anchor must scroll
    // up to make room while older output scrolls off.
    term.send("one<A-Enter>two");
    term.expect_screen(
        "fill-005\n\
         fill-006\n\
         fill-007\n\
         fill-008\n\
         fill-009\n\
         fill-010\n\
         tst> one\n\
         ::: two",
    );
    term.send("<A-Enter>three");
    term.expect_screen(
        "fill-006\n\
         fill-007\n\
         fill-008\n\
         fill-009\n\
         fill-010\n\
         tst> one\n\
         ::: two\n\
         ::: three",
    );

    // The whole buffer must survive submission intact.
    term.send("<Enter>");
    term.expect_contains("GOT: one");
    term.quit();
}

#[test]
fn long_line_wraps_and_scrolls_at_bottom() {
    let term = TestTerm::builder().size(6, 20).spawn();
    term.send(":fill 6<Enter>");
    term.expect_line(5, "tst>");

    // 5 + 30 = 35 cells at 20 cols -> 2 rows; the prompt row must scroll up
    // by one and rendering must stay coherent.
    term.send(&"w".repeat(30));
    term.expect_line(4, &format!("tst> {}", "w".repeat(15)));
    term.expect_line(5, &"w".repeat(15));
    term.send("<Enter>");
    term.expect_contains("GOT: www");
    term.quit();
}

#[test]
fn wide_chars_wrapping_at_bottom_keep_anchor_consistent() {
    let term = TestTerm::builder().size(6, 20).spawn();
    term.send(":fill 6<Enter>");
    term.expect_line(5, "tst>");

    // Each emoji is 2 cells: 5 + 2*9 = 23 cells at 20 cols -> wraps to a
    // second row at the bottom of the screen. A width-estimation or anchor
    // error shows up as a duplicated prompt row or clobbered filler line.
    term.send(&"😊".repeat(9));
    term.expect_line(4, &format!("tst> {}", "😊".repeat(7)));
    term.expect_line(5, &"😊".repeat(2));

    // The screen above the prompt must still be exactly the scrolled filler.
    term.expect_line(3, "fill-006");
    term.send("<Enter>");
    term.expect_contains("GOT: 😊");
    term.quit();
}
