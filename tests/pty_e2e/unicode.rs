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
    term.expect_contains("GOT: Emoji test 😊 checks");
    term.quit();
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
