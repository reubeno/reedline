//! Unicode and emoji: wide cells, cursor movement over graphemes, deletion
//! (UX checklist "Unicode and Emojis").

use crate::harness::TestTerm;

#[test]
fn emoji_occupies_two_cells_and_cursor_advances_past_it() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("😊");
    term.expect_screen("tst> 😊");
    term.expect_cursor(0, 7);
    term.expect("emoji cell to be wide", |screen| {
        let cell = screen.cell(0, 5).ok_or("no cell at (0, 5)")?;
        if cell.is_wide() {
            Ok(())
        } else {
            Err(format!("cell contents {:?} not wide", cell.contents()))
        }
    });
    term.quit_after_clear();
}

#[test]
fn backspace_deletes_whole_emoji() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("a😊<BS>");
    term.expect_screen("tst> a");
    term.expect_cursor(0, 6);
    term.quit_after_clear();
}

#[test]
fn arrows_move_over_wide_cells_grapheme_wise() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("a😊b");
    term.expect_cursor(0, 9);
    term.send("<Left>");
    term.expect_cursor(0, 8); // before 'b'
    term.send("<Left>");
    term.expect_cursor(0, 6); // before the emoji: skips both cells at once
    term.send("X");
    term.expect_screen("tst> aX😊b");
    term.quit_after_clear();
}

#[test]
fn checklist_emoji_line_round_trips() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    // The exact line from UX_TESTING.md, pasted to arrive as one unit.
    // ZWJ-sequence width differs across width tables, so assert round-trip
    // fidelity (Home/End + submit echo) rather than exact cursor columns.
    let line = "Emoji test 😊 checks 🤦🏼‍♂️ unicode";
    term.paste(line);
    term.expect_contains("Emoji test 😊 checks");
    term.send("<Home>");
    term.expect_cursor(0, 5);
    term.send("<End><Enter>");
    // Fidelity check: the ZWJ cluster and the text after it survive the
    // round trip, not just the prefix.
    term.expect_contains("GOT: Emoji test 😊 checks");
    term.expect("submitted line keeps the ZWJ emoji and tail", |screen| {
        let rows = crate::harness::screen_rows(screen);
        match rows.iter().find(|r| r.starts_with("GOT: ")) {
            Some(row) if row.contains('🤦') && row.ends_with("unicode") => Ok(()),
            Some(row) => Err(format!("GOT row mangled: {row:?}")),
            None => Err("no GOT row".into()),
        }
    });
    term.quit();
}

#[test]
fn emoji_straddling_wrap_boundary_moves_whole_glyph() {
    let term = TestTerm::builder().size(6, 10).spawn();
    term.expect_cursor(0, 5);
    // "abcd" ends at col 8; the emoji needs cols 9..11 but only col 9
    // remains: the whole glyph must wrap, never split across rows.
    term.send("abcd😊");
    term.expect_line(0, "tst> abcd");
    term.expect_line(1, "😊");
    term.expect_cursor(1, 2);
    term.send("<BS>!");
    term.expect_screen("tst> abcd!");
    term.quit_after_clear();
}

#[test]
fn home_and_end_remain_accurate_with_wide_chars() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("日本語");
    term.expect_cursor(0, 11); // 3 wide chars = 6 cells
    term.send("<Home>");
    term.expect_cursor(0, 5);
    term.send("<End>!");
    term.expect_screen("tst> 日本語!");
    term.expect_cursor(0, 12);
    term.quit_after_clear();
}
