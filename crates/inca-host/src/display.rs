// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The failure to reach the display server the environment names, before the app launches.

use std::any::Any;
use std::ffi::OsStr;
use std::sync::atomic::{AtomicBool, Ordering};

/// Set once the app has launched.
static LAUNCHED: AtomicBool = AtomicBool::new(false);

/// The variable that names the display server `gpui` connects to: `WAYLAND_DISPLAY` before
/// `DISPLAY`. An empty variable names none.
fn display_variable(wayland: Option<&OsStr>, x11: Option<&OsStr>) -> Option<&'static str> {
    let named = |value: Option<&OsStr>| value.is_some_and(|value| !value.is_empty());
    if named(wayland) {
        Some("WAYLAND_DISPLAY")
    } else if named(x11) {
        Some("DISPLAY")
    } else {
        None
    }
}

/// The line that reports that the display server `variable` names cannot be reached, for the
/// failure `cause`.
fn unreachable_display(variable: &str, cause: &str) -> String {
    format!(
        concat!(
            "cannot reach the display server that {variable} names: {cause}. ",
            "unset {variable} to run without a window, or point it at a running server"
        ),
        variable = variable,
        cause = cause,
    )
}

/// The line to print for a panic with `payload`, or `None` when the panic keeps its own report.
/// A panic before the launch, while a display variable is set, is taken for an unreachable display
/// server: the platform connects before the launch and panics when it cannot.
fn report(
    launched: bool,
    wayland: Option<&OsStr>,
    x11: Option<&OsStr>,
    payload: &(dyn Any + Send),
) -> Option<String> {
    let variable = display_variable(wayland, x11).filter(|_| !launched)?;
    let cause = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("the connection failed");
    Some(unreachable_display(variable, cause))
}

/// Installs a panic hook that reports a panic before [`launched`], while a display variable is
/// set, as an unreachable display server on stderr and exits with status 1. Every other panic
/// goes to the hook installed before.
pub(crate) fn exit_on_unreachable_display() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let line = report(
            LAUNCHED.load(Ordering::Relaxed),
            std::env::var_os("WAYLAND_DISPLAY").as_deref(),
            std::env::var_os("DISPLAY").as_deref(),
            info.payload(),
        );
        match line {
            Some(line) => {
                eprintln!("{line}");
                std::process::exit(1);
            }
            None => previous(info),
        }
    }));
}

/// Marks the app as launched, so later panics keep their own report.
pub(crate) fn launched() {
    LAUNCHED.store(true, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::wayland(Some("wayland-0"), Some(":0"), Some("WAYLAND_DISPLAY"))]
    #[case::x11(None, Some(":0"), Some("DISPLAY"))]
    #[case::empty_wayland(Some(""), Some(":0"), Some("DISPLAY"))]
    #[case::none(None, None, None)]
    #[case::empty_x11(None, Some(""), None)]
    fn the_display_variable_is_the_one_gpui_connects_through(
        #[case] wayland: Option<&str>,
        #[case] x11: Option<&str>,
        #[case] want: Option<&str>,
    ) {
        assert_eq!(
            display_variable(wayland.map(OsStr::new), x11.map(OsStr::new)),
            want
        );
    }

    #[test]
    fn the_report_names_the_variable_and_the_cause() {
        assert_eq!(
            unreachable_display("DISPLAY", "boom"),
            concat!(
                "cannot reach the display server that DISPLAY names: boom. ",
                "unset DISPLAY to run without a window, or point it at a running server"
            )
        );
    }

    #[rstest]
    #[case::str(Box::new("boom"), "boom")]
    #[case::string(Box::new(String::from("owned")), "owned")]
    #[case::other(Box::new(7_u8), "the connection failed")]
    fn the_report_carries_the_panic_message(
        #[case] payload: Box<dyn Any + Send>,
        #[case] cause: &str,
    ) {
        assert_eq!(
            report(false, Some(OsStr::new("wayland-0")), None, &*payload),
            Some(unreachable_display("WAYLAND_DISPLAY", cause))
        );
    }

    #[rstest]
    #[case::launched(true, Some("wayland-0"))]
    #[case::no_display(false, None)]
    fn a_panic_keeps_its_own_report(#[case] launched: bool, #[case] wayland: Option<&str>) {
        assert_eq!(
            report(launched, wayland.map(OsStr::new), None, &"boom"),
            None
        );
    }

    #[test]
    fn launched_sets_the_flag() {
        launched();
        assert!(LAUNCHED.load(Ordering::Relaxed));
    }
}
