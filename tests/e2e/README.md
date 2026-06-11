# End-to-end terminal tests

These tests drive a real reedline instance inside a pseudo-terminal and
assert on an emulated VT100 screen. They automate the manual checklist in
[`UX_TESTING.md`](../../UX_TESTING.md) and cover painter behavior — anchor
tracking, scrolling, resizing, wide glyphs, styled cells, cursor-position
(DSR) round-trips — that unit tests cannot observe.

## Architecture

Three pieces, each with detailed docs in its own header:

- **Fixture** ([`examples/e2e_fixture.rs`](../../examples/e2e_fixture.rs)) —
  a deterministic reedline REPL. Features (hinter, menus, highlighter, vi
  mode, history, transient prompt, …) are switched on per-test via `FIX_*`
  environment variables, and submitted `:commands` simulate host behavior
  (printing filler, partial lines, external-printer messages, DSR queries).
  The fixture is the only reedline consumer in the picture; tests configure
  it instead of constructing `Reedline` directly.
- **Harness** ([`harness.rs`](harness.rs)) — `TestTerm` spawns the fixture
  under a PTY (`portable-pty`), pumps its output through an in-memory screen
  (`vt100`), and plays the terminal's side of the conversation: it answers
  the child's `CSI 6n` cursor-position queries from the emulated cursor and
  counts them, so tests can assert *rendering* and *query budgets* with the
  same object.
- **Key notation** ([`keys.rs`](keys.rs)) — input is written vim-style:
  `term.send("abc<Left><C-w><A-Enter><Esc>")`.

Scenario files are one-per-feature-area; `main.rs` lists them.

## Assertion model

Assertions follow Neovim's `screen.expect()` design: every `expect_*` helper
re-evaluates against the **eventual** screen state after each chunk of child
output, until it matches or a timeout expires. Tests therefore never sleep
to "wait for" output, and they are immune to repaint timing. The main
helpers:

- `expect_screen` — pins the entire visible grid (the strongest and
  preferred assertion; failures print a want/got per-row diff),
- `expect_line` / `expect_cursor_line` (the row the cursor is on) /
  `expect_contains` / `expect_cursor` / custom predicates via `expect`,
- `expect_fg` — styled-cell colors; use the `RED`/`GREEN`/`DARK_GRAY`
  constants, which are kept in sync with the fixture's styles,
- `expect_unchanged` — the negative form: the screen must *not* change
  within a (CI-scaled) window (Neovim's `unchanged`),
- `dsr_count` — how many cursor-position queries the child has issued;
  `dsr.rs` pins a budget for every paint path.

Prefer the exact helpers over `expect_contains`: a contains-check is often
satisfied by the typed command's echo or a stale row (e.g. `"ping"` matches
`tst> :ext ping` before the message ever prints).

All timing derives from one knob and is scaled ×3 when `CI` is set;
`E2E_TIMEOUT_MS` overrides the base.

## Determinism rules (read before writing a test)

- `spawn()` blocks until the first prompt is painted — never race fixture
  startup.
- When the effect of one keystroke depends on the *previous* keystroke's
  repaint, assert between sends. Reedline batches events that arrive in one
  read, and some features (hint completion, abbreviation expansion) only
  see state computed at the previous repaint. Sending `"he<C-f>"` as one
  string exercises batching; `send("he")`, assert, `send("<C-f>")`
  exercises the interactive path. Choose deliberately.
- A bare `<Esc>` automatically pauses before subsequent bytes so the child
  does not parse `ESC x` as an Alt-chord. Intentional Alt-chords use
  `<A-x>`.
- In vi mode, end tests with `quit_from_vi()` — plain `quit()` types
  `:quit`, which is swallowed by normal mode.
- Each test owns its own PTY and process: the suite is fully parallel and
  nextest-compatible. The harness builds the fixture via cargo on first use
  (a no-op when fresh).

### Worked example: a bottom-of-screen scroll test

Filler-line arithmetic is the most error-prone part of authoring. The
recipe: after `:fill N<Enter>` on a fresh prompt, the terminal has printed
`1` command-echo row + `N` filler rows + `1` new prompt row = `N + 2`
lines; an `R`-row screen shows the **last `R`** of them. So on an 8-row
screen, `:fill 10` leaves `fill-004 … fill-010` (7 rows) above the prompt:

```rust
let term = TestTerm::builder().size(8, 30).spawn();
term.expect_cursor(0, 5);          // never race startup
term.send(":fill 10<Enter>");
term.expect_screen(
    "fill-004\n\
     fill-005\n\
     fill-006\n\
     fill-007\n\
     fill-008\n\
     fill-009\n\
     fill-010\n\
     tst> ",
);
```

Every subsequent row of buffer growth (wrap, Alt-Enter line, menu, hint)
scrolls one more filler row off the top. Exemplars to copy from: simple
screen test — `basics::typing_echoes_and_enter_submits`; scroll math —
`scrolling::growing_multiline_buffer_scrolls_anchor_up`; DSR budget —
`dsr::buffer_editor_costs_exactly_one_dsr`; feature-gated module —
`external_printer.rs`.

## Running

```sh
cargo test --test e2e --features external_printer   # Unix only
```

Known-bug reproductions live in `issue_repros.rs`, verified to fail and
marked `#[ignore = "reproduces nushell/reedline#N"]` so the suite stays
green. Run them with `cargo test --test e2e -- --ignored`; when an upstream
fix lands, the repro passes and removing the attribute turns it into a
regression guard.

## Checklist coverage

Every section of `UX_TESTING.md` is automated: Basics and Clearing
(`basics.rs`, `clearing.rs`), Unicode/Emoji (`unicode.rs`), History
(`history.rs`), Syntax highlighting (`highlighting.rs`, via styled-cell
colors), and the checklist's TODO sections, Completion and VI mode
(`menus.rs`, `hints.rs`, `vi_mode.rs`). One pinned discrepancy: the
checklist describes `Ctrl-b`/`Ctrl-f` as word motions, but reedline's emacs
defaults bind them to char moves (word motions are `Alt-b`/`Alt-f`,
`Ctrl-Left`/`Ctrl-Right`); see
`basics::ctrl_b_f_move_by_char_and_alt_b_f_by_word`.

Beyond the checklist, scenario files cover resize, scrolling at the bottom
row, buffers taller than the screen, multi-line editing, selection via
shift-modified movement, right prompts, host output (including
no-trailing-newline), the external buffer editor, the external printer,
break-signal suspension, transient prompts, kill-ring/undo, abbreviations,
degenerate terminal sizes, and DSR budgets.

## Platform and emulator caveats

- Unix only for now; Windows ConPTY support would slot in at the
  `portable-pty` layer.
- The `vt100` crate panics on single-row screens, so there is no 1-row
  test (noted in `edge_sizes.rs`).
- ZWJ emoji sequences can differ in width between the emulator and the
  `unicode-width` version reedline compiles against; tests assert
  round-trip fidelity for those rather than exact cursor columns.
