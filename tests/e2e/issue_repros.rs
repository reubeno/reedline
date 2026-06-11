//! Reproductions of open nushell/reedline issues.
//!
//! Tests that demonstrate a live bug are marked
//! `#[ignore = "reproduces nushell/reedline#N"]` so the suite stays green;
//! run them with `cargo test -- --ignored` (or nextest
//! `--run-ignored ignored-only`). When an issue is fixed, its test starts
//! passing under `--ignored`: remove the attribute to turn it into a
//! permanent regression guard.

use crate::harness::{screen_rows, TestTerm};

/// nushell/reedline#999: recalling a multi-line history entry should place
/// the cursor at the end of the *buffer*, not the end of the first line.
#[test]
#[ignore = "reproduces nushell/reedline#999 — cursor lands at end of first line"]
fn issue_999_multiline_recall_cursor_at_end_of_buffer() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send("ls<A-Enter>| where type == file<Enter>");
    term.expect_contains("GOT: ls");
    term.send("<Up>");
    term.expect_contains("::: | where type == file");
    // Conventional position: end of the last line.
    term.expect_cursor(3, 24);
    term.quit_after_clear();
}

/// nushell/reedline#860: a host command that prints `\e[6n` (DSR) receives
/// the terminal's reply *outside* read_line; the reply must not surface as
/// `^[[N;NR` text in the next prompt.
///
/// Observed in the harness: the stale CPR desynchronizes the input stream —
/// reedline pairs its own query with the stale reply, and the fresh reply
/// is then consumed as key events, corrupting subsequent typed input.
#[test]
#[ignore = "reproduces nushell/reedline#860 — stale CPR reply corrupts subsequent input"]
fn issue_860_host_dsr_reply_not_echoed_into_prompt() {
    let term = TestTerm::spawn();
    term.expect_cursor(0, 5);
    term.send(":dsr<Enter>");
    // The terminal's CPR reply (e.g. "\x1b[2;1R") is now queued on stdin.
    // The next prompt must not contain it as buffer text.
    term.expect("prompt free of CPR residue", |screen| {
        let (row, _) = screen.cursor_position();
        let text = screen_rows(screen)
            .get(row as usize)
            .cloned()
            .unwrap_or_default();
        if text.contains(';') && text.ends_with('R') {
            return Err(format!("CPR leaked into the buffer: {text:?}"));
        }
        if text == "tst>" {
            Ok(())
        } else {
            Err(format!("cursor row reads {text:?}"))
        }
    });
    // And a subsequent submission must round-trip cleanly.
    term.send("clean<Enter>");
    term.expect_contains("GOT: clean");
    term.quit();
}

/// nushell/reedline#974: a hint long enough to wrap beyond the screen must
/// not make the prompt line invisible.
///
/// Observed in the harness: the wrapped hint fills every screen row and the
/// prompt+input line scrolls out of view entirely.
#[test]
#[ignore = "reproduces nushell/reedline#974 — giant hint scrolls prompt off screen"]
fn issue_974_long_hint_keeps_prompt_visible() {
    let long = format!("x {}", "word ".repeat(60)); // ~300 cells
    let term = TestTerm::builder()
        .size(6, 20)
        .env("FIX_HINT", "1")
        .history(&[long.as_str()])
        .spawn();
    term.expect_cursor(0, 5);
    term.send("x");
    // The hint wraps far past the 6-row screen; whatever is shown, the
    // prompt+input row must remain visible.
    term.expect("prompt row visible despite giant hint", |screen| {
        let rows = screen_rows(screen);
        if rows.iter().any(|r| r.starts_with("tst> x")) {
            Ok(())
        } else {
            Err(format!("prompt not visible: {rows:?}"))
        }
    });
    term.quit_after_clear();
}

/// nushell/reedline#1005: a burst of ExternalPrinter messages must not wipe
/// unrelated screen content.
///
/// Observed in the harness: after a 5-message burst the entire screen is
/// cleared — prior host output, command echoes, and all messages — leaving
/// only a bare prompt at row 0 (the issue's "full screen wipes").
#[cfg(feature = "external_printer")]
#[test]
#[ignore = "reproduces nushell/reedline#1005 — message burst wipes the whole screen"]
fn issue_1005_external_printer_burst_does_not_wipe_screen() {
    let term = TestTerm::builder().size(12, 40).spawn();
    term.expect_cursor(0, 5);
    term.send(":fill 3<Enter>");
    term.expect_contains("fill-003");
    term.send(":extn 5 msg<Enter>");
    term.expect_contains("msg-5");
    // All five messages and the prior host output must coexist.
    for needle in ["fill-001", "fill-002", "fill-003", "msg-1", "msg-3"] {
        term.expect_contains(needle);
    }
    term.send("ok<Enter>");
    term.expect_contains("GOT: ok");
    term.quit();
}
