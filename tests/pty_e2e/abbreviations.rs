//! Fish-style abbreviations: expansion ordering relative to the typed space
//! (issue #1072) and expand-on-Enter (issue #1083).

use crate::harness::TestTerm;

fn abbr_term() -> TestTerm {
    TestTerm::builder()
        .env("FIX_ABBR", "gco=git checkout")
        .spawn()
}

#[test]
fn abbreviation_expands_before_typed_space() {
    let term = abbr_term();
    term.expect_cursor(0, 5);
    // Sync before sending the space: the engine only expands when the
    // space is the first edit in its event batch (engine.rs ~1352), so a
    // space coalesced into the same read as "gco" skips expansion. (That
    // batch-sensitivity is itself a candidate upstream bug: fast/batched
    // input -- e.g. over SSH -- loses space-expansion.)
    term.send("gco");
    term.expect_screen("tst> gco");
    term.send("<Space>");
    // #1072: the space lands after the expansion, never mid-expansion.
    term.expect_screen("tst> git checkout");
    term.expect_cursor(0, 18);
    term.send("main<Enter>");
    term.expect_contains("GOT: git checkout main");
    term.quit();
}

#[test]
fn abbreviation_expands_on_enter() {
    let term = abbr_term();
    term.expect_cursor(0, 5);
    term.send("gco<Enter>");
    // #1083: Enter both expands and submits.
    term.expect_contains("GOT: git checkout");
    term.quit();
}

#[test]
fn non_abbreviation_word_is_untouched() {
    let term = abbr_term();
    term.expect_cursor(0, 5);
    term.send("gcox<Space>done<Enter>");
    term.expect_contains("GOT: gcox done");
    term.quit();
}
