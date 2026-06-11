//! End-to-end tests that drive reedline inside a real PTY.
//!
//! See `pty_e2e/harness.rs` for the harness and `examples/pty_fixture.rs`
//! for the fixture REPL under test. Unix-only for now (Windows ConPTY is a
//! possible follow-up).
#![cfg(unix)]

mod harness;
mod keys;

mod abbreviations;
mod basics;
mod buffer_editor;
mod clearing;
mod dsr;
mod edge_sizes;
#[cfg(feature = "external_printer")]
mod external_printer;
mod highlighting;
mod hints;
mod history;
mod host_output;
mod issue_repros;
mod kill_ring;
mod large_buffer;
mod menus;
mod resize;
mod right_prompt;
mod scrolling;
mod suspension;
mod transient;
mod unicode;
mod vi_mode;
