//! Vi edit mode basics (UX checklist "VI mode").
//!
//! The fixture's prompt indicator renders `[i] ` / `[n] ` so mode changes
//! are observable on screen — tests synchronize on the indicator after
//! `<Esc>` instead of relying on timing.

use crate::harness::TestTerm;

#[test]
fn insert_then_normal_mode_edits() {
    let term = TestTerm::builder().vi_mode().spawn();
    term.expect_line(0, "tst> [i]");
    term.expect_cursor(0, 9);
    // Insert mode by default: type, then Esc to normal mode.
    term.send("abc<Esc>");
    term.expect_contains("[n]");
    // `0` jumps to line start, `x` deletes the char under the cursor.
    term.send("0x");
    term.expect_screen("tst> [n] bc");
    // Vi mode (normal) persists across read_line calls: abort, then
    // re-enter insert mode so `:quit` is typed as text.
    term.send("<C-c>");
    term.expect_fresh_prompt();
    term.send("i");
    term.expect_contains("[i]");
    term.quit();
}

#[test]
fn normal_mode_word_motions_and_append() {
    let term = TestTerm::builder().vi_mode().spawn();
    term.expect_cursor(0, 9);
    term.send("one two three<Esc>");
    term.expect_contains("[n]");
    // `b` back a word, `dw` delete word forward; the space before "three"
    // stays, as in vi.
    term.send("bdw");
    term.expect_screen("tst> [n] one two");
    term.expect_cursor(0, 17);
    // `A` appends at end of line (after the trailing space).
    term.send("A!<Esc>");
    term.expect_contains("[n]");
    term.expect_screen("tst> [n] one two !");
    term.send("<Enter>");
    term.expect_contains("GOT: one two !");
    // Submitting resets vi to insert mode for the next line (unlike Ctrl-C,
    // which preserves normal mode), so :quit can be typed directly.
    term.expect_contains("[i]");
    term.quit();
}

#[test]
fn mode_indicator_round_trips() {
    let term = TestTerm::builder().vi_mode().spawn();
    term.expect_line(0, "tst> [i]");
    term.send("<Esc>");
    term.expect_line(0, "tst> [n]");
    term.send("i");
    term.expect_line(0, "tst> [i]");
    term.quit();
}
