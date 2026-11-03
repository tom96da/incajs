// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Where the log files of an app are and what they are called.

use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::entry::civil;

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

/// The directory the app with `identifier` writes its log files to.
pub(crate) fn log_dir(identifier: Option<&str>) -> PathBuf {
    let base = base_dir(
        cfg!(target_os = "macos"),
        std::env::home_dir().as_deref(),
        std::env::var_os("XDG_STATE_HOME").as_deref(),
        &std::env::temp_dir(),
    );
    base.join(dir_name(identifier))
}

/// The file name stem of a run that starts at `secs`: `YYYYMMDDTHHMMSSZ`.
pub(super) fn file_stem(secs: u64) -> String {
    let (year, month, day, hour, minute, second) = civil(secs);
    format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}Z")
}

/// The number of log files that stay in the directory.
const KEEP: usize = 10;

/// Whether `name` is `<stem>.log`: eight digits, `T`, six digits, `Z`, `.log`.
/// The digits are not range checked.
fn is_log_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() == 20
        && bytes[..8].iter().all(u8::is_ascii_digit)
        && bytes[8] == b'T'
        && bytes[9..15].iter().all(u8::is_ascii_digit)
        && bytes[15] == b'Z'
        && &bytes[16..] == b".log"
}

/// The oldest of `names` beyond the newest [`KEEP`] files, never `current`.
fn stale(names: &[String], current: &str) -> Vec<String> {
    let Some(excess) = names.len().checked_sub(KEEP) else {
        return Vec::new();
    };
    let mut others: Vec<&String> = names.iter().filter(|name| *name != current).collect();
    others.sort();
    others.into_iter().take(excess).cloned().collect()
}

/// The log files in `dir` to delete when `current` is the file of this run.
/// A directory or a link whose name has the shape of a log file stays out.
pub(super) fn list_stale(dir: &Path, current: &str) -> io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && let Some(name) = entry.file_name().to_str()
            && is_log_name(name)
        {
            names.push(name.to_owned());
        }
    }
    Ok(stale(&names, current))
}

/// Deletes `names` in `dir`. A failed removal prints `run log not pruned.
/// <io error>` on `stderr` and the others still go.
pub(super) fn remove_all(dir: &Path, names: &[String], stderr: &mut impl Write) {
    for name in names {
        if let Err(err) = fs::remove_file(dir.join(name)) {
            let _ = writeln!(stderr, "run log not pruned. {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::unnecessary_wraps)]

    use rstest::rstest;

    use super::*;
    use crate::tests::{ScratchDir, names_of, old_names};

    /// 2001-02-03T04:05:06Z.
    const START: u64 = 981_173_106;

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

    #[test]
    fn a_file_stem_pads_every_field_to_its_width() {
        assert_eq!(file_stem(START), "20010203T040506Z");
    }

    #[rstest]
    #[case::valid("20261102T163000Z.log", true)]
    #[case::panic_log("panic.log", false)]
    #[case::wrong_extension("20261102T163000Z.txt", false)]
    #[case::shorter_than_the_time_digits("20261102T1234", false)]
    #[case::letter_among_the_digits("2026110aT163000Z.log", false)]
    #[case::letter_among_the_time_digits("20261102T16300aZ.log", false)]
    #[case::lowercase_t("20261102t163000Z.log", false)]
    #[case::lowercase_z("20261102T163000z.log", false)]
    #[case::backup("20261102T163000Z.log.bak", false)]
    fn a_log_name_is_a_start_time_stem_and_log(#[case] name: &str, #[case] expected: bool) {
        assert_eq!(is_log_name(name), expected);
    }

    #[rstest]
    #[case::none(0, 0)]
    #[case::ten(10, 0)]
    #[case::eleven(11, 1)]
    #[case::fifteen(15, 5)]
    fn the_oldest_names_beyond_ten_are_stale(#[case] count: usize, #[case] stale_count: usize) {
        let names = old_names(count);
        let current = names.last().cloned().unwrap_or_default();

        assert_eq!(stale(&names, &current), names[..stale_count]);
    }

    #[test]
    fn the_current_file_is_never_stale_when_the_clock_moved_back() {
        let names = old_names(12);

        assert_eq!(stale(&names, &names[0]), names[1..3]);
    }

    /// Writes an empty file for each of `names` into `dir`.
    fn touch_all(dir: &Path, names: &[String]) {
        for name in names {
            fs::write(dir.join(name), "").unwrap();
        }
    }

    #[test]
    fn pruning_deletes_the_oldest_log_files() {
        let dir = ScratchDir::new("run-log-prune");
        let names = old_names(12);
        touch_all(&dir, &names);

        let stale = list_stale(&dir, &names[11]).unwrap();
        remove_all(&dir, &stale, &mut Vec::new());

        assert_eq!(names_of(&dir), names[2..]);
    }

    #[test]
    fn pruning_leaves_a_directory_named_like_a_log() {
        let dir = ScratchDir::new("run-log-prune-dir");
        let names = old_names(12);
        touch_all(&dir, &names);
        fs::create_dir(dir.join("20000101T000001Z.log")).unwrap();

        assert_eq!(list_stale(&dir, &names[11]).unwrap(), names[..2]);
    }

    #[test]
    fn pruning_leaves_a_file_that_only_ends_in_log() {
        let dir = ScratchDir::new("run-log-prune-notes");
        let names = old_names(11);
        touch_all(&dir, &names);
        fs::write(dir.join("notes.log"), "").unwrap();

        assert_eq!(list_stale(&dir, &names[10]).unwrap(), names[..1]);
    }

    #[test]
    fn a_directory_that_cannot_be_read_gives_an_error() {
        let dir = ScratchDir::new("run-log-missing");

        assert!(list_stale(&dir.join("missing"), "").is_err());
    }

    #[test]
    fn a_link_named_like_a_log_is_not_deleted() {
        let dir = ScratchDir::new("run-log-link");
        let target = dir.join("target.txt");
        fs::write(&target, "").unwrap();
        let link = "20000101T000000Z.log";
        std::os::unix::fs::symlink(&target, dir.join(link)).unwrap();
        let mut all = old_names(11)[1..].to_vec();
        let current = "20000112T000000Z.log".to_owned();
        all.push(current.clone());
        touch_all(&dir, &all);

        let stale = list_stale(&dir, &current).unwrap();
        remove_all(&dir, &stale, &mut Vec::new());

        assert!(fs::symlink_metadata(dir.join(link)).is_ok());
    }

    /// Removes `missing-1`, `present` and `missing-2` from a directory that holds
    /// `present`, and returns the directory and what went to stderr.
    fn remove_with_failures(name: &str) -> (ScratchDir, String) {
        let dir = ScratchDir::new(name);
        fs::write(dir.join("present"), "").unwrap();
        let names = ["missing-1", "present", "missing-2"].map(str::to_owned);
        let mut stderr = Vec::new();
        remove_all(&dir, &names, &mut stderr);
        (dir, String::from_utf8(stderr).unwrap())
    }

    #[test]
    fn a_failed_removal_prints_one_line_for_each_failure() {
        let (_dir, stderr) = remove_with_failures("run-log-remove-lines");

        let lines: Vec<_> = stderr.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(
            lines
                .iter()
                .all(|line| line.starts_with("run log not pruned. "))
        );
    }

    #[test]
    fn a_failed_removal_does_not_stop_the_others() {
        let (dir, _stderr) = remove_with_failures("run-log-remove-others");

        assert!(!dir.join("present").exists());
    }
}
