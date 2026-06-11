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
fn bracketed_paste_inserts_text() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.paste("pasted text");
    term.expect_screen("tst> pasted text");
    term.send("<Enter>");
    term.expect_contains("GOT: pasted text");
    term.quit();
}
