// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Writes the panic of a packaged app to a file the person who runs it can find.

use std::backtrace::{Backtrace, BacktraceStatus};
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The file a panic is written to, inside the directory of the app.
const LOG_FILE: &str = "panic.log";
/// The directory name of an app whose identifier is missing or is not a plain
/// file name.
const FALLBACK_DIR: &str = "inca-app";

/// The directory that holds the log directory of every app: `<home>/Library/Logs`
/// on macOS, else `$XDG_STATE_HOME` when that is an absolute path, else
/// `<home>/.local/state`. `temp` when there is no home to use.
fn base_dir(macos: bool, home: Option<&Path>, state: Option<&OsStr>, temp: &Path) -> PathBuf {
    let home = home.filter(|home| !home.as_os_str().is_empty());
    if macos {
        return home.map_or_else(|| temp.to_owned(), |home| home.join("Library").join("Logs"));
    }
    state
        .map(PathBuf::from)
        .filter(|state| state.is_absolute())
        .or_else(|| home.map(|home| home.join(".local").join("state")))
        .unwrap_or_else(|| temp.to_owned())
}

/// The directory name of an app: its identifier when that is a plain file name.
fn dir_name(identifier: Option<&str>) -> &str {
    identifier
        .filter(|id| {
            !id.is_empty()
                && !id.starts_with('.')
                && id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        })
        .unwrap_or(FALLBACK_DIR)
}

/// Where the app with `identifier` writes its panic.
pub(crate) fn log_path(identifier: Option<&str>) -> PathBuf {
    let base = base_dir(
        cfg!(target_os = "macos"),
        std::env::home_dir().as_deref(),
        std::env::var_os("XDG_STATE_HOME").as_deref(),
        &std::env::temp_dir(),
    );
    base.join(dir_name(identifier)).join(LOG_FILE)
}

/// The text of the log: the host version, the thread and the panic, then the
/// backtrace when there is one.
fn report(version: &str, thread: &str, info: &str, backtrace: Option<&str>) -> String {
    let mut text = format!("inca-host {version}\nthread '{thread}' {info}\n");
    if let Some(backtrace) = backtrace {
        text.push_str("stack backtrace:\n");
        text.push_str(backtrace);
        text.push('\n');
    }
    text
}

/// Replaces the file at `path` with `text`, creating its directories.
fn write(path: &Path, text: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, text)
}

/// The backtrace of the panic that runs the hook, or `None` when the
/// environment asks for none.
fn captured_backtrace() -> Option<String> {
    let backtrace = Backtrace::capture();
    (backtrace.status() == BacktraceStatus::Captured).then(|| backtrace.to_string())
}

/// Writes `text` to `path`. A failure goes to `stderr` as
/// `panic log not written. <io error>`.
fn log_panic(path: &Path, text: &str, stderr: &mut impl Write) {
    if let Err(err) = write(path, text) {
        let _ = writeln!(stderr, "panic log not written. {err}");
    }
}

/// Installs a panic hook that writes the panic to `path`, then runs the hook
/// installed before.
pub(crate) fn install(path: PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let text = report(
            env!("CARGO_PKG_VERSION"),
            thread.name().unwrap_or("<unnamed>"),
            &info.to_string(),
            captured_backtrace().as_deref(),
        );
        log_panic(&path, &text, &mut io::stderr());
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::unnecessary_wraps)]

    use rstest::rstest;

    use super::*;
    use crate::tests::ScratchDir;

    fn os(text: &str) -> Option<&OsStr> {
        Some(OsStr::new(text))
    }

    fn home(text: &str) -> Option<&Path> {
        Some(Path::new(text))
    }

    #[rstest]
    #[case::macos_home(true, home("/Users/a"), None, "/Users/a/Library/Logs")]
    #[case::macos_ignores_the_state_directory(
        true,
        home("/Users/a"),
        os("/state"),
        "/Users/a/Library/Logs"
    )]
    #[case::macos_without_home(true, None, os("/state"), "/tmp")]
    #[case::linux_state_directory(false, home("/home/a"), os("/state"), "/state")]
    #[case::linux_state_directory_without_home(false, None, os("/state"), "/state")]
    #[case::linux_relative_state_directory(
        false,
        home("/home/a"),
        os("state"),
        "/home/a/.local/state"
    )]
    #[case::linux_empty_state_directory(false, home("/home/a"), os(""), "/home/a/.local/state")]
    #[case::linux_unset_state_directory(false, home("/home/a"), None, "/home/a/.local/state")]
    #[case::linux_empty_home(false, home(""), None, "/tmp")]
    #[case::linux_nothing(false, None, None, "/tmp")]
    fn the_log_directory_follows_the_platform_and_the_environment(
        #[case] macos: bool,
        #[case] home: Option<&Path>,
        #[case] state: Option<&OsStr>,
        #[case] expected: &str,
    ) {
        assert_eq!(
            base_dir(macos, home, state, Path::new("/tmp")),
            PathBuf::from(expected)
        );
    }

    #[rstest]
    #[case::reverse_dns(Some("org.inca.demo"), "org.inca.demo")]
    #[case::dash_and_underscore(Some("my-app_2"), "my-app_2")]
    #[case::missing(None, "inca-app")]
    #[case::empty(Some(""), "inca-app")]
    #[case::leading_dot(Some(".hidden"), "inca-app")]
    #[case::parent_directory(Some(".."), "inca-app")]
    #[case::slash(Some("a/b"), "inca-app")]
    #[case::space(Some("a b"), "inca-app")]
    #[case::non_ascii(Some("caf\u{e9}"), "inca-app")]
    fn the_directory_of_an_app_is_its_identifier_when_that_is_a_plain_file_name(
        #[case] identifier: Option<&str>,
        #[case] expected: &str,
    ) {
        assert_eq!(dir_name(identifier), expected);
    }

    #[rstest]
    #[case::without_a_backtrace(
        None,
        "inca-host 0.0.24\nthread 'main' panicked at a.rs:1:2:\nboom\n"
    )]
    #[case::with_a_backtrace(
        Some("0: f"),
        "inca-host 0.0.24\nthread 'main' panicked at a.rs:1:2:\nboom\nstack backtrace:\n0: f\n"
    )]
    fn a_report_names_the_version_the_thread_and_the_panic(
        #[case] backtrace: Option<&str>,
        #[case] expected: &str,
    ) {
        assert_eq!(
            report("0.0.24", "main", "panicked at a.rs:1:2:\nboom", backtrace),
            expected
        );
    }

    #[test]
    fn a_panic_log_is_created_with_its_directories() {
        let dir = ScratchDir::new("panic-log-create");
        let path = dir.join("org.inca.demo").join("panic.log");

        write(&path, "boom").unwrap();

        assert_eq!(fs::read_to_string(path).unwrap(), "boom");
    }

    #[test]
    fn a_panic_log_replaces_the_previous_one() {
        let dir = ScratchDir::new("panic-log-replace");
        let path = dir.join("panic.log");
        write(&path, "first").unwrap();

        write(&path, "second").unwrap();

        assert_eq!(fs::read_to_string(path).unwrap(), "second");
    }

    #[test]
    fn a_panic_log_under_a_file_prints_the_io_error_on_stderr() {
        let dir = ScratchDir::new("panic-log-blocked");
        fs::write(dir.join("file"), "").unwrap();
        let path = dir.join("file").join("panic.log");
        let err = write(&path, "boom").unwrap_err();
        let mut stderr = Vec::new();

        log_panic(&path, "boom", &mut stderr);

        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            format!("panic log not written. {err}\n")
        );
    }
}
