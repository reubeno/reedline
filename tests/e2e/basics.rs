//! Prompt rendering, echoing, line editing, and multi-line buffers.

use crate::harness::TestTerm;

#[test]
fn prompt_renders_and_cursor_sits_after_it() {
    let term = TestTerm::spawn();
    term.expect_screen("tst> ");
    term.expect_cursor(0, 5);
    term.quit();
}

#[test]
fn typing_echoes_and_enter_submits() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("hello");
    term.expect_screen("tst> hello");
    term.send("<Enter>");
    term.expect_screen(
        "tst> hello\n\
         GOT: hello\n\
         tst> ",
    );
    term.expect_cursor(2, 5);
    term.quit();
}

#[test]
fn arrow_keys_edit_mid_line() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("ac<Left>b");
    term.expect_screen("tst> abc");
    term.expect_cursor(0, 7);
    term.send("<End>!");
    term.expect_screen("tst> abc!");
    term.expect_cursor(0, 9);
    term.send("<Home>#");
    term.expect_screen("tst> #abc!");
    term.expect_cursor(0, 6);
    term.quit_after_clear();
}

#[test]
fn word_navigation_and_word_delete() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("alpha beta gamma");
    // Jump to the start of "gamma" and insert.
    term.send("<C-Left>X");
    term.expect_screen("tst> alpha beta Xgamma");
    // Ctrl-W deletes the word fragment before the cursor.
    term.send("<C-w>");
    term.expect_screen("tst> alpha beta gamma");
    term.quit_after_clear();
}

#[test]
fn backspace_and_delete() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("abcd<BS>");
    term.expect_screen("tst> abc");
    term.send("<Home><Del>");
    term.expect_screen("tst> bc");
    term.quit_after_clear();
}

#[test]
fn multiline_prompt_renders_above_input_line() {
    let term = TestTerm::builder().prompt("info line\\ntst> ").spawn();
    term.expect_screen(
        "info line\n\
         tst> ",
    );
    term.expect_cursor(1, 5);
    term.send("hi<Enter>");
    term.expect_screen(
        "info line\n\
         tst> hi\n\
         GOT: hi\n\
         info line\n\
         tst> ",
    );
    term.expect_cursor(4, 5);
    term.quit();
}

#[test]
fn multiline_buffer_uses_continuation_prompt() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("first<A-Enter>second");
    term.expect_screen(
        "tst> first\n\
         ::: second",
    );
    term.send("<Enter>");
    term.expect_screen(
        "tst> first\n\
         ::: second\n\
         GOT: first\n\
         second\n\
         tst> ",
    );
    term.quit();
}

#[test]
fn mixed_case_typing_echoes_exactly() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("Hello World MIXED case");
    term.expect_screen("tst> Hello World MIXED case");
    term.send("<Enter>");
    term.expect_contains("GOT: Hello World MIXED case");
    term.quit();
}

#[test]
fn ctrl_b_f_move_by_char_and_alt_b_f_by_word() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("one two");
    // Reedline's emacs defaults: Ctrl-b/Ctrl-f are CHAR moves (the manual
    // checklist text says "word", but word motions are Alt-b/Alt-f or
    // Ctrl-Left/Ctrl-Right).
    term.send("<C-b><C-b>");
    term.expect_cursor(0, 10);
    term.send("<C-f>");
    term.expect_cursor(0, 11);
    term.send("<A-b>");
    term.expect_cursor(0, 9); // start of "two"
    term.send("<A-f>");
    term.expect_cursor(0, 12);
    term.quit_after_clear();
}

#[test]
fn ctrl_d_on_empty_prompt_exits_cleanly() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("<C-d>");
    term.expect_eof();
}

#[test]
fn ansi_colored_prompt_keeps_cursor_math() {
    let term = TestTerm::builder()
        .prompt("\u{1b}[31mred>\u{1b}[0m ")
        .spawn();
    term.expect_line(0, "red>");
    term.expect_fg(0, 0..4, vt100::Color::Idx(1));
    // Escape bytes must not count toward the prompt width.
    term.expect_cursor(0, 5);
    term.send("x");
    term.expect_cursor(0, 6);
    term.expect_line(0, "red> x");
    term.quit_after_clear();
}

#[test]
fn paste_with_newlines_becomes_multiline_buffer_not_submissions() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.paste("one\ntwo\nthree");
    // Bracketed paste must insert, never submit (documented contract of
    // use_bracketed_paste).
    term.expect_screen(
        "tst> one\n\
         ::: two\n\
         ::: three",
    );
    term.expect_unchanged(crate::harness::unchanged_window());
    term.send("<Enter>");
    term.expect_contains("GOT: one");
    term.expect_contains("two");
    term.quit();
}

#[test]
fn bracketed_paste_inserts_text() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.paste("pasted text");
    term.expect_screen("tst> pasted text");
    term.send("<Enter>");
    term.expect_contains("GOT: pasted text");
    term.quit();
}
