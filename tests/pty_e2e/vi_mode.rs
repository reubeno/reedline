//! Vi edit mode basics (UX checklist "VI mode").

use crate::harness::TestTerm;

#[test]
fn insert_then_normal_mode_edits() {
    let term = TestTerm::builder().vi_mode().spawn();
    term.expect_cursor(0, 5);
    // Insert mode by default: type, then Esc to normal mode.
    term.send("abc<Esc>");
    term.expect_screen("tst> abc");
    // `0` jumps to line start, `x` deletes the char under the cursor.
    term.send("0x");
    term.expect_screen("tst> bc");
    // Vi mode (normal) persists across read_line calls: abort, then
    // re-enter insert mode so `:quit` is typed as text.
    term.send("<C-c>");
    term.expect_contains("(ctrl-c)");
    term.send("i");
    term.quit();
}

#[test]
fn normal_mode_word_motions_and_append() {
    let term = TestTerm::builder().vi_mode().spawn();
    term.expect_cursor(0, 5);
    term.send("one two three<Esc>");
    // `b` back a word, `dw` delete word forward; the space before "three"
    // stays, as in vi.
    term.send("bdw");
    term.expect_screen("tst> one two");
    term.expect_cursor(0, 13);
    // `A` appends at end of line (after the trailing space).
    term.send("A!<Esc>");
    term.expect_screen("tst> one two !");
    term.send("<Enter>");
    term.expect_contains("GOT: one two !");
    // Submitting resets vi to insert mode for the next line (unlike Ctrl-C,
    // which preserves normal mode), so :quit can be typed directly.
    term.quit();
}
