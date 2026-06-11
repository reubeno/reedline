//! History hints: dimmed completion text painted after the cursor
//! (UX checklist "completion/hinting"). The fixture's hinter uses dark gray
//! (palette index 8).

use crate::harness::TestTerm;

fn hinting_term() -> TestTerm {
    TestTerm::builder()
        .env("FIX_HINT", "1")
        .history(&["hello world"])
        .spawn()
}

#[test]
fn hint_renders_dimmed_after_cursor() {
    let term = hinting_term();
    term.expect_cursor(0, 5);
    term.send("he");
    // Buffer "he" + hint "llo world" — the full line shows on screen but
    // the cursor stays after the typed text.
    term.expect_line(0, "tst> hello world");
    term.expect_cursor(0, 7);
    term.expect_fg(0, 7..16, vt100::Color::Idx(8));
    term.quit_after_clear();
}

#[test]
fn hint_is_not_part_of_submission_and_leaves_no_residue() {
    let term = hinting_term();
    term.expect_cursor(0, 5);
    term.send("he<Enter>");
    // Only the typed text submits; the hint must vanish from the submitted
    // line rather than linger as painted residue.
    term.expect_screen(
        "tst> he\n\
         GOT: he\n\
         tst> ",
    );
    term.quit();
}

#[test]
fn ctrl_f_accepts_hint() {
    let term = hinting_term();
    term.expect_cursor(0, 5);
    // Wait for the hint to render before accepting: events batched in one
    // read are processed against the previous repaint's hint state.
    term.send("he");
    term.expect_line(0, "tst> hello world");
    term.send("<C-f>");
    term.expect_cursor(0, 16); // cursor after accepted hint
    term.send("<Enter>");
    term.expect_contains("GOT: hello world");
    term.quit();
}

#[test]
fn hint_clears_when_prefix_diverges() {
    let term = hinting_term();
    term.expect_cursor(0, 5);
    term.send("he");
    term.expect_line(0, "tst> hello world");
    term.send("x"); // "hex" matches nothing
    term.expect_screen("tst> hex");
    term.quit_after_clear();
}

#[test]
fn hint_wrapping_at_bottom_row_scrolls_cleanly() {
    let term = TestTerm::builder()
        .size(6, 20)
        .env("FIX_HINT", "1")
        .history(&["abcdefghij klmnopqrst"])
        .spawn();
    term.send(":fill 6<Enter>");
    term.expect_line(5, "tst>");
    // Typing the prefix paints a hint that wraps past the bottom row; the
    // painter must scroll the anchor up rather than truncate or clobber.
    term.send("abcde");
    term.expect_contains("abcdefghij");
    term.send("<C-f><Enter>");
    term.expect_contains("GOT: abcdefghij");
    term.quit();
}
