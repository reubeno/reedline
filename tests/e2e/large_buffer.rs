//! Buffers taller than the screen: the painter's `large_buffer` path
//! (`required_lines >= screen_height`), which anchors at row 0 and skips
//! lines from the top via `skip_buffer_lines`. Entirely distinct repaint
//! logic from the common small-buffer path.

use crate::harness::{screen_rows, TestTerm};

#[test]
fn single_wrapped_line_taller_than_screen_stays_editable() {
    let term = TestTerm::builder().size(6, 20).spawn();
    term.expect_cursor(0, 5);
    // 5 + 150 cells -> 8 rows at 20 cols: exceeds the 6-row screen.
    term.send(&"z".repeat(150));
    term.expect_contains(&"z".repeat(20)); // some full wrapped row visible
                                           // Editing at the end must stay coherent.
    term.send("<BS><BS>!");
    term.expect_contains("zz!");
    // Submit echo: "GOT: " + 148 z's + "!" = 154 cells = 8 rows; the head
    // scrolls off the 6-row window.
    term.send("<Enter>");
    term.expect_screen(&format!(
        "{z20}\n{z20}\n{z20}\n{z20}\n{z13}!\ntst> ",
        z20 = "z".repeat(20),
        z13 = "z".repeat(13),
    ));
    term.quit();
}

#[test]
fn multiline_buffer_taller_than_screen_skips_top_lines() {
    let term = TestTerm::builder().size(6, 30).spawn();
    term.expect_cursor(0, 5);
    term.send("l1");
    for i in 2..=8 {
        term.send(&format!("<A-Enter>l{i}"));
    }
    // The cursor line (l8) must be visible; the top of the buffer must have
    // scrolled out of view.
    term.expect_contains("::: l8");
    term.expect("top buffer lines to be skipped", |screen| {
        let rows = screen_rows(screen);
        if rows.iter().any(|r| r == "tst> l1") {
            Err("first buffer line still on screen".into())
        } else {
            Ok(())
        }
    });
    // No line may be duplicated by the skip arithmetic.
    term.expect("no duplicated buffer lines", |screen| {
        let rows = screen_rows(screen);
        let mut seen = std::collections::HashSet::new();
        for r in rows.iter().filter(|r| !r.is_empty()) {
            if !seen.insert(r.clone()) {
                return Err(format!("row {r:?} appears twice"));
            }
        }
        Ok(())
    });
    // Submitting echoes all 8 lines; "GOT: l1" scrolls off the 6-row
    // window, leaving the echo tail and a fresh prompt.
    term.send("<Enter>");
    term.expect_screen(
        "l4\n\
         l5\n\
         l6\n\
         l7\n\
         l8\n\
         tst> ",
    );
    term.quit();
}

#[test]
fn buffer_exactly_filling_screen_keeps_every_line() {
    let term = TestTerm::builder().size(6, 30).spawn();
    term.expect_cursor(0, 5);
    // 1 prompt line + 5 continuations = exactly 6 rows = screen height,
    // the boundary where the large-buffer path switches on.
    term.send("a1<A-Enter>a2<A-Enter>a3<A-Enter>a4<A-Enter>a5<A-Enter>a6");
    term.expect_screen(
        "tst> a1\n\
         ::: a2\n\
         ::: a3\n\
         ::: a4\n\
         ::: a5\n\
         ::: a6",
    );
    // Repaints at the boundary must stay stable.
    term.send("<BS>X");
    term.expect_line(5, "::: aX");
    term.expect_line(0, "tst> a1");
    // The submit echo scrolls "GOT: a1" off the 6-row window.
    term.send("<Enter>");
    term.expect_screen(
        "a2\n\
         a3\n\
         a4\n\
         a5\n\
         aX\n\
         tst> ",
    );
    term.quit();
}

#[test]
fn menu_on_buffer_taller_than_screen_paints_at_bottom() {
    let term = TestTerm::builder().size(6, 30).completion_menu().spawn();
    term.expect_cursor(0, 5);
    term.send("x1");
    for i in 2..=7 {
        term.send(&format!("<A-Enter>x{i}"));
    }
    // Cursor is far from the prompt: the menu must claim the bottom rows
    // (the large_buffer_offset layout path).
    term.send("<A-Enter>al<Tab>");
    term.expect_contains("alphabet");
    // The submit echo (8 lines) scrolls the head off the 6-row window.
    term.send("<Esc><Enter>");
    term.expect_screen(
        "x4\n\
         x5\n\
         x6\n\
         x7\n\
         al\n\
         tst> ",
    );
    term.quit();
}
