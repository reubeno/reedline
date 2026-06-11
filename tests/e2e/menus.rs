//! Completion menu painting: opening, selection, and menus near the bottom
//! of the screen (which force the painter to scroll the prompt anchor).

use crate::harness::{screen_rows, TestTerm};

#[test]
fn tab_opens_menu_with_candidates() {
    let term = TestTerm::builder().size(10, 40).completion_menu().spawn();
    term.expect_cursor(0, 5);
    term.send("al<Tab>");
    // Exact layout: ColumnarMenu paints its "| " marker after the prompt
    // and the candidates in fixed-width columns on the row below.
    term.expect_screen(
        "tst> | al\n\
         alpha     alphabet  alphard",
    );
    term.quit_after_clear();
}

#[test]
fn ide_menu_renders_unicode_candidates_with_description() {
    let term = TestTerm::builder()
        .size(12, 40)
        .env("FIX_MENU", "ide")
        .spawn();
    term.expect_cursor(0, 5);
    term.send("h<Tab>");
    // Candidates with combining accents and the selected candidate's
    // description must both render (issues #998/#996 were width bugs here).
    term.expect_contains("héllo");
    term.expect_contains("héllium");
    term.expect_contains("greeting");
    term.send("<Enter>");
    term.expect_line(0, "tst> héllo");
    term.send("<Enter>");
    term.expect_contains("GOT: héllo");
    term.quit();
}

#[test]
fn ide_menu_handles_wide_glyph_candidates() {
    let term = TestTerm::builder()
        .size(12, 40)
        .env("FIX_MENU", "ide")
        .spawn();
    term.expect_cursor(0, 5);
    term.send("日<Tab>");
    term.expect_contains("日本語x");
    term.send("<Enter>");
    term.expect_line(0, "tst> 日本語x");
    term.quit_after_clear();
}

#[test]
fn partial_completion_inserts_common_prefix() {
    let term = TestTerm::builder()
        .size(10, 40)
        .completion_menu()
        .env("FIX_PARTIAL", "1")
        .spawn();
    term.expect_cursor(0, 5);
    // Common prefix of alpha/alphabet/alphard is "alpha" (#1001 class).
    term.send("al<Tab>");
    term.expect_contains("alphabet");
    term.expect("buffer to hold the common prefix", |screen| {
        let row = &screen_rows(screen)[0];
        if row.contains("alpha") {
            Ok(())
        } else {
            Err(format!("row 0: {row:?}"))
        }
    });
    term.quit_after_clear();
}

#[test]
fn enter_accepts_selected_candidate() {
    let term = TestTerm::builder().size(10, 40).completion_menu().spawn();
    term.expect_cursor(0, 5);
    term.send("be<Tab>");
    // The menu opens (ColumnarMenu paints its "| " marker after the prompt)
    // with the sole candidate listed below.
    term.expect_line(0, "tst> | be");
    term.expect_line(1, "beta");
    // Enter accepts the selected candidate into the buffer and closes the
    // menu; a second Enter submits.
    term.send("<Enter>");
    term.expect_screen("tst> beta");
    term.send("<Enter>");
    term.expect_contains("GOT: beta");
    term.quit();
}

#[test]
fn escape_closes_menu_and_restores_clean_screen() {
    let term = TestTerm::builder().size(10, 40).completion_menu().spawn();
    term.expect_cursor(0, 5);
    term.send("al<Tab>");
    term.expect_contains("alphabet");
    term.send("<Esc>");
    term.expect("menu to close leaving only the prompt line", |screen| {
        let rows = screen_rows(screen);
        if rows[1..].iter().all(|r| r.is_empty()) {
            Ok(())
        } else {
            Err(format!("rows below prompt not empty: {:?}", &rows[1..]))
        }
    });
    term.quit_after_clear();
}

#[test]
fn menu_at_bottom_scrolls_prompt_up() {
    let term = TestTerm::builder().size(8, 40).completion_menu().spawn();
    term.send(":fill 10<Enter>");
    term.expect_line(7, "tst>");

    term.send("al<Tab>");
    // Menu rows need space below the prompt: the anchor must scroll up and
    // the candidates must be visible.
    term.expect_contains("alphabet");
    term.expect("prompt line to scroll above the menu", |screen| {
        let rows = screen_rows(screen);
        // The menu marker ("| ") sits between prompt and buffer.
        match rows
            .iter()
            .position(|r| r.starts_with("tst>") && r.contains("al"))
        {
            Some(r) if r < 7 => Ok(()),
            Some(r) => Err(format!("prompt still at bottom row {r}")),
            None => Err(format!("prompt line not found: {rows:?}")),
        }
    });
    term.quit_after_clear();
}
