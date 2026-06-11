//! Cut-buffer (kill ring) and undo/redo, observed at the terminal level:
//! buffer shrink/grow repaints must leave no residue.

use crate::harness::TestTerm;

#[test]
fn kill_line_then_yank_round_trips() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("alpha beta<Home><C-k>"); // KillLine cuts to end of line
    term.expect_screen("tst> ");
    term.expect_cursor(0, 5);
    term.send("<C-y>"); // PasteCutBufferBefore
    term.expect_screen("tst> alpha beta");
    term.send("<Enter>");
    term.expect_contains("GOT: alpha beta");
    term.quit();
}

#[test]
fn cut_from_start_shrinks_painted_line() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("disposable kept<A-b><C-u>"); // cut everything before "kept"
    term.expect_screen("tst> kept");
    term.expect_cursor(0, 5);
    term.send("<End><C-y>"); // yank it back at the end
    term.expect_screen("tst> keptdisposable ");
    term.quit_after_clear();
}

#[test]
fn undo_and_redo_repaint_correctly() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("hello world<C-w>"); // cut last word
    term.expect_screen("tst> hello ");
    term.send("<C-z>"); // Undo restores in one press (issue #956 class)
    term.expect_screen("tst> hello world");
    term.send("<C-g>"); // Redo
    term.expect_screen("tst> hello ");
    term.quit_after_clear();
}
