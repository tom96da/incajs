// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Where the app's `console` writes: stderr, colored as the environment asks.

use std::ffi::OsStr;
use std::io::IsTerminal;
use std::rc::Rc;

use inca_jsenv::console::{Level, Output};

/// A console [`Output`] writing each line to stderr.
#[must_use]
pub(crate) fn to_stderr() -> Output {
    Rc::new(|_: Level, message: &str| eprintln!("{message}"))
}

/// The color decision for the given variables and terminal state.
fn color_enabled(
    no_color: Option<&OsStr>,
    force_color: Option<&OsStr>,
    stderr_is_terminal: bool,
) -> bool {
    if no_color.is_some_and(|value| !value.is_empty()) {
        return false;
    }
    match force_color {
        Some(value) if value == "0" || value == "false" => false,
        Some(value) if !value.is_empty() => true,
        _ => stderr_is_terminal,
    }
}

/// Whether `console` output to stderr should be colored, as the environment
/// asks.
///
/// Color is off when `NO_COLOR` is non-empty, which wins over `FORCE_COLOR`.
/// Otherwise it is on when `FORCE_COLOR` is set to anything except `0`,
/// `false` or the empty string, and otherwise on when stderr is a terminal.
/// Pass the result to [`inca_jsenv::console::install`].
#[must_use]
pub(crate) fn color_from_env() -> bool {
    color_enabled(
        std::env::var_os("NO_COLOR").as_deref(),
        std::env::var_os("FORCE_COLOR").as_deref(),
        std::io::stderr().is_terminal(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(text: &str) -> &OsStr {
        OsStr::new(text)
    }

    #[test]
    fn a_non_empty_no_color_turns_color_off_whatever_else_is_set() {
        for force in [
            None,
            Some(os("")),
            Some(os("1")),
            Some(os("0")),
            Some(os("true")),
        ] {
            for terminal in [false, true] {
                assert!(!color_enabled(Some(os("1")), force, terminal));
                assert!(!color_enabled(Some(os("anything")), force, terminal));
            }
        }
    }

    #[test]
    fn an_empty_no_color_is_ignored() {
        assert!(color_enabled(Some(os("")), None, true));
        assert!(!color_enabled(Some(os("")), None, false));
        assert!(color_enabled(Some(os("")), Some(os("1")), false));
    }

    #[test]
    fn force_color_zero_or_false_turns_color_off_on_a_terminal() {
        assert!(!color_enabled(None, Some(os("0")), true));
        assert!(!color_enabled(None, Some(os("false")), true));
        assert!(!color_enabled(None, Some(os("0")), false));
        assert!(!color_enabled(None, Some(os("false")), false));
    }

    #[test]
    fn any_other_non_empty_force_color_turns_color_on_without_a_terminal() {
        for value in ["1", "2", "3", "true", "yes", "FALSE", "00"] {
            assert!(color_enabled(None, Some(os(value)), false), "{value}");
            assert!(color_enabled(None, Some(os(value)), true), "{value}");
        }
    }

    #[test]
    fn without_either_variable_the_terminal_decides() {
        assert!(color_enabled(None, None, true));
        assert!(!color_enabled(None, None, false));
    }
}
