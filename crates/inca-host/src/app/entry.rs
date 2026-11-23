// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The process entries: running a bundle in a window and printing the config of an app.

use std::fs;
use std::path::Path;
use std::process::ExitCode;
use std::rc::Rc;

use gpui::App;
use gpui_platform::application;

use inca_jsenv::{EngineError, ErrorReporter, stderr_reporter};

use super::session::{Failure, Session, clock_of};
use super::window::start;
use crate::config::{self, effective_config};
use crate::dev::{
    SharedWriter, StdoutWriter, fail_startup, install_panic_hook, reporter_for, serve_dev_protocol,
    stdin_lines,
};
use crate::run_log;

/// Runs the bundle at `entry_path` in a GPUI window and blocks until the
/// app quits. Returns `ExitCode::SUCCESS` after a normal quit.
///
/// In production the entry evaluates to completion before the window opens.
/// With `dev` set, the host also reads rebuild and HMR messages from stdin
/// and reports errors to the parent process. The entry then runs while the
/// host serves those messages, and the window opens once the entry's
/// top-level `await` settles or two seconds pass. While the app reports a
/// failed load, the window stays closed.
///
/// A bundle that fails to start and an entry that cannot be read both exit
/// the process with status 1. In production the failure prints to stderr and
/// is written to the log file of the run; with `dev` set the failure is
/// reported as a throwing first bundle. On Linux, a `DISPLAY` or
/// `WAYLAND_DISPLAY` that names a server the host cannot reach exits the
/// process with status 1 and one line on stderr that names the variable.
///
/// An error the running app raises and a panic of a packaged app are written to
/// the log file of the run in the log directory of the app. With `dev` set, a
/// panic goes to the parent process as a `-32603` error.
#[must_use]
pub fn run_bundle(entry_path: &str, dev: bool) -> ExitCode {
    let writer: Option<SharedWriter> = if dev {
        let stdout_writer = StdoutWriter::spawn();
        let sender = stdout_writer.sender_clone();
        install_panic_hook(sender, stdout_writer.thread_id());
        Some(Rc::new(stdout_writer) as SharedWriter)
    } else {
        let identifier = config::try_read(Path::new(entry_path))
            .ok()
            .and_then(|config| config.identifier);
        run_log::install(
            run_log::log_dir(identifier.as_deref()),
            run_log::start_secs(),
        );
        None
    };
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    crate::display::exit_on_unreachable_display();
    let source = match fs::read_to_string(entry_path) {
        Ok(source) => source,
        Err(err) => fail_startup(
            &Failure::bundle(format!("failed to read {entry_path}: {err}")),
            writer.as_ref(),
        ),
    };
    let entry_path = entry_path.to_owned();

    let closing = writer.clone();
    application().run(move |cx: &mut App| {
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        crate::display::launched();
        // macOS keeps an app alive with no windows left; this framework's
        // apps are single-window, so closing the window is quitting.
        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        if let Some(writer) = writer.clone() {
            close_writer_on_quit(cx, writer);
        }

        let error_reporter = writer.as_ref().map_or_else(
            || {
                let base = stderr_reporter();
                Rc::new(move |err: &EngineError| {
                    run_log::error("error", &err.to_string());
                    base(err);
                }) as ErrorReporter
            },
            reporter_for,
        );
        if let Some(writer) = &writer {
            let lines = stdin_lines();
            let started = cx.background_executor().now();
            match Session::begin_with(
                &entry_path,
                &source,
                error_reporter,
                Some(writer),
                clock_of(cx.background_executor()),
            ) {
                Ok(first) => serve_dev_protocol(
                    cx,
                    lines,
                    first,
                    started,
                    entry_path.clone(),
                    Rc::clone(writer),
                ),
                Err(err) => fail_startup(&Failure::Thrown(err), Some(writer)),
            }
        } else if let Err(failure) = start(cx, &entry_path, &source, error_reporter) {
            fail_startup(&failure, None);
        }
    });

    if let Some(writer) = closing {
        writer.close();
    }
    ExitCode::SUCCESS
}

// macOS ends the process inside `quit`, so `run` never returns to close the writer.
pub(super) fn close_writer_on_quit(cx: &mut App, writer: SharedWriter) {
    cx.on_app_quit(move |_| {
        writer.close();
        async {}
    })
    .detach();
}

/// The settings the host applies to the app whose entry is `entry_path`,
/// read from the `inca.json` beside it, or the message to report when
/// `entry_path` names no file or that `inca.json` cannot be read or parsed.
fn config_report(entry_path: &str) -> Result<String, String> {
    if !Path::new(entry_path).is_file() {
        return Err(format!("failed to read {entry_path}: not a file"));
    }
    let config = config::try_read(Path::new(entry_path))
        .map_err(|err| format!("failed to read {}", err.report()))?;
    if let Some((path, reason)) = config::unusable(&config) {
        let file = config::config_path(Path::new(entry_path)).unwrap_or_default();
        return Err(format!(
            "failed to read {}: {path}: {reason}",
            file.display()
        ));
    }
    Ok(effective_config(&config))
}

/// Prints the settings the host applies to the app whose entry is
/// `entry_path`, read from the `inca.json` beside it, and returns after
/// printing them. Returns `ExitCode::FAILURE` and prints nothing on stdout
/// when `entry_path` names no file or that `inca.json` cannot be read or
/// parsed, or a value in it that no window can use; the line on stderr then
/// names the file, and for a value of the wrong type, broken JSON or an
/// unusable value the path to it.
#[must_use]
pub fn print_config(entry_path: &str) -> ExitCode {
    match config_report(entry_path) {
        Ok(report) => {
            println!("{report}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    use crate::tests::ScratchDir;

    #[test]
    fn the_report_of_a_missing_entry_names_it() {
        let dir = ScratchDir::new("print-missing");
        let entry = dir.join("bundle.js").to_string_lossy().into_owned();

        assert_eq!(
            config_report(&entry),
            Err(format!("failed to read {entry}: not a file"))
        );
    }

    #[test]
    fn the_report_reads_the_config_beside_the_entry() {
        let dir = ScratchDir::new("print-present");
        let entry = dir.join("bundle.js");
        fs::write(&entry, "").unwrap();
        fs::write(dir.join("inca.json"), r#"{"name":"Demo"}"#).unwrap();

        let report: serde_json::Value =
            serde_json::from_str(&config_report(&entry.to_string_lossy()).unwrap()).unwrap();
        assert_eq!(report["name"], "Demo");
    }

    #[test]
    fn the_report_of_a_config_it_cannot_parse_names_the_field() {
        let dir = ScratchDir::new("print-malformed");
        let entry = dir.join("bundle.js");
        fs::write(&entry, "").unwrap();
        fs::write(dir.join("inca.json"), r#"{"window":{"width":"wide"}}"#).unwrap();

        let message = config_report(&entry.to_string_lossy()).unwrap_err();

        let config = dir.join("inca.json");
        assert!(
            message.starts_with(&format!(
                r#"failed to read {}: ["window","width"]: "#,
                config.display()
            )),
            "{message}"
        );
    }

    #[test]
    fn config_report_refuses_an_unusable_value_with_its_path() {
        let dir = ScratchDir::new("print-unusable");
        let entry = dir.join("bundle.js");
        fs::write(&entry, "").unwrap();
        fs::write(dir.join("inca.json"), r#"{"window":{"width":-100}}"#).unwrap();

        assert_eq!(
            config_report(&entry.to_string_lossy()),
            Err(format!(
                concat!(
                    r#"failed to read {}: ["window","width"]: "#,
                    "invalid value: -100, expected a finite number above zero"
                ),
                dir.join("inca.json").display()
            ))
        );
    }
}
