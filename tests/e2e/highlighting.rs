//! Syntax highlighting: styled-cell assertions (UX checklist "Syntax
//! highlighting"). The fixture's highlighter colors the configured keyword
//! `test` green and non-matching words red.

use crate::harness::TestTerm;

#[test]
fn matching_keyword_renders_green() {
    let term = TestTerm::builder().highlighter().spawn();
    term.expect_cursor(0, 5);
    term.send("test");
    term.expect_screen("tst> test");
    term.expect_fg(0, 5..9, vt100::Color::Idx(2)); // green
    term.quit_after_clear();
}

#[test]
fn non_matching_word_renders_red() {
    let term = TestTerm::builder().highlighter().spawn();
    term.expect_cursor(0, 5);
    term.send("tex");
    term.expect_screen("tst> tex");
    term.expect_fg(0, 5..8, vt100::Color::Idx(1)); // red
    term.quit_after_clear();
}

#[test]
fn highlight_updates_as_word_completes() {
    let term = TestTerm::builder().highlighter().spawn();
    term.expect_cursor(0, 5);
    term.send("tes");
    term.expect_fg(0, 5..8, vt100::Color::Idx(1)); // still a non-match
    term.send("t");
    term.expect_fg(0, 5..9, vt100::Color::Idx(2)); // completes the keyword
    term.send("<BS>");
    term.expect_fg(0, 5..8, vt100::Color::Idx(1)); // back to non-match
    term.quit_after_clear();
}
