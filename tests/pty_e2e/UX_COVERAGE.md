# UX_TESTING.md → automated coverage map

Every manual check in `UX_TESTING.md` and the PTY e2e test(s) that automate
it. Tests live in `tests/pty_e2e/`; run with
`cargo test --test pty_e2e --features external_printer` (Unix).

## Basics

| Manual check | Test(s) |
| --- | --- |
| Typing a short line with upper- and lowercase | `basics::mixed_case_typing_echoes_exactly` |
| Movement left/right using arrow keys | `basics::arrow_keys_edit_mid_line` |
| Word left / word right | `basics::ctrl_b_f_move_by_char_and_alt_b_f_by_word`, `basics::word_navigation_and_word_delete` — note: the checklist says `Ctrl-b`/`Ctrl-f` move by *word*, but reedline's emacs defaults bind them to *char* moves; word motions are `Alt-b`/`Alt-f` and `Ctrl-Left`/`Ctrl-Right`. The test pins actual behavior; the checklist text should be corrected. |
| `Enter` to complete entry | `basics::typing_echoes_and_enter_submits` |

### Clearing

| Manual check | Test(s) |
| --- | --- |
| `Ctrl-c` aborts to empty prompt below | `clearing::ctrl_c_aborts_to_fresh_prompt_below` |
| `Ctrl-l` clears screen, keeps entry, entry submits | `clearing::ctrl_l_moves_prompt_to_top_and_preserves_buffer`, plus `_with_multiline_buffer`, `_when_prompt_already_at_top_is_idempotent`, `_then_further_editing_stays_coherent`; DSR cost pinned by `dsr::clear_screen_costs_exactly_one_dsr` |

### Unicode and Emojis

| Manual check | Test(s) |
| --- | --- |
| Paste the emoji test line, move cursor over emojis | `unicode::checklist_emoji_line_round_trips`, `unicode::arrows_move_over_wide_cells_grapheme_wise` |
| Delete the smiley | `unicode::backspace_deletes_whole_emoji` |
| `Home`/`End` at accurate positions | `unicode::home_and_end_remain_accurate_with_wide_chars` |
| Emoji line can be entered | `unicode::checklist_emoji_line_round_trips` |
| (extra) wide glyph at wrap boundary | `unicode::emoji_straddling_wrap_boundary_moves_whole_glyph`, `scrolling::wide_chars_wrapping_at_bottom_keep_anchor_consistent` |

## History

| Manual check | Test(s) |
| --- | --- |
| Up-arrow recalls previous entry | `history::up_arrow_recalls_most_recent_entry_first` |
| Re-running a recalled entry is not duplicated; leave recall with down-arrow | `history::resubmitting_recalled_entry_does_not_duplicate_history`, `history::up_arrow_recalls_most_recent_entry_first` (down-arrow leg) |
| Typed prefix + up-arrow finds matching entry | `history::prefix_typed_then_up_finds_matching_entry` |
| `Ctrl-r` reverse search; more hits via `Ctrl-r` | `history::ctrl_r_reverse_search_finds_and_aborts`, `history::ctrl_r_cycles_through_multiple_matches`, `history::ctrl_r_enter_accepts_match_into_buffer` |
| Abort search with `Ctrl-c` | `history::ctrl_r_reverse_search_finds_and_aborts` (note: exits the whole `read_line`; from search, the pre-search buffer is restored — pinned in `dsr::menu_and_history_search_issue_no_dsr`) |
| (extra) multi-line entries, exclusion prefix | `history::multiline_history_entry_recalls_and_searches`, `history::exclusion_prefix_keeps_entry_out_of_history` |

## Syntax highlighting

| Manual check | Test(s) |
| --- | --- |
| Entering `test` highlights it differently | `highlighting::matching_keyword_renders_green`, `_non_matching_word_renders_red`, `_highlight_updates_as_word_completes` (styled-cell color assertions) |

## Completion (checklist TODO — covered anyway)

`menus::*` (columnar open/select/escape, exact layouts, menu-forced scroll at
bottom, IDE menu with unicode + descriptions, partial completions),
`hints::*` (render style, accept via `Ctrl-f`, no-submit, divergence,
wrap-at-bottom), `large_buffer::menu_on_buffer_taller_than_screen_paints_at_bottom`.

## VI mode (checklist TODO — covered anyway)

`vi_mode::*` (mode indicator round-trip, normal-mode editing, word motions,
append; mode persistence across Ctrl-C vs reset on submit).

## Beyond the checklist

Painter-focused scenarios with no manual-checklist counterpart: resize
(`resize::*`), scrolling/anchor (`scrolling::*`), buffers taller than the
screen (`large_buffer::*`), right prompts (`right_prompt::*`), host output
incl. no-trailing-newline (`host_output::*`), external editor
(`buffer_editor::*`), external printer (`external_printer::*`), suspension
via break signal (`suspension::*`), transient prompts (`transient::*`),
kill-ring/undo (`kill_ring::*`), abbreviations (`abbreviations::*`),
degenerate sizes (`edge_sizes::*`), DSR round-trip budgets (`dsr::*`), and
live-bug reproductions (`issue_repros::*`, `#[ignore]`d until fixed
upstream; run with `--ignored`).
