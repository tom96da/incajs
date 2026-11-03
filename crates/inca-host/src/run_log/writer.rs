// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The log file of a run: when it is created, what it holds and how it is written.

use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::SystemTime;

use super::entry::{Time, captured_backtrace, entry, header, now, report};
use super::files::{file_stem, list_stale, remove_all};

/// The most bytes of entries one run writes.
const MAX_RUN: usize = 1024 * 1024;
/// The number of start times a run tries before it gives up on a name.
const ATTEMPTS: u64 = 10;
/// The body of the entry that replaces the first entry past the run cap.
const NOTICE: &str = "log full. later errors are not recorded";

/// The log file of one run.
struct RunLog {
    dir: PathBuf,
    start_secs: u64,
    header: String,
    /// `None` until the first entry creates the file.
    file: Option<File>,
    /// The bytes of entries and repair newlines written or attempted, which the run
    /// cap counts.
    written: usize,
    /// Set when the notice replaced an entry. Later `error` and `startup`
    /// entries are dropped.
    full: bool,
    /// Set when a write failed and may have left half a line. The next write
    /// starts with a newline.
    torn: bool,
}

impl RunLog {
    /// A log that writes into `dir` and names its file after `start_secs`. It
    /// touches the disk at the first [`RunLog::record`].
    fn new(dir: PathBuf, start_secs: u64) -> Self {
        Self {
            dir,
            start_secs,
            header: header(
                option_env!("INCA_HOST_PACKAGE"),
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::consts::ARCH,
            ),
            file: None,
            written: 0,
            full: false,
            torn: false,
        }
    }

    /// Creates the directory and the file. A name taken by another run moves
    /// the start time on by a second. Returns the file and its name.
    fn create(&self) -> io::Result<(File, String)> {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.dir)?;
        let mut taken = io::Error::from(io::ErrorKind::AlreadyExists);
        for attempt in 0..ATTEMPTS {
            let name = format!("{}.log", file_stem(self.start_secs + attempt));
            match OpenOptions::new()
                .append(true)
                .create_new(true)
                .mode(0o600)
                .open(self.dir.join(&name))
            {
                Ok(file) => return Ok((file, name)),
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => taken = err,
                Err(err) => return Err(err),
            }
        }
        Err(taken)
    }

    /// Appends `text`, the built entry of `kind`, to the file of the run. The
    /// first call creates the file, writes the header and prunes old files.
    /// A failure prints one line on `stderr` and the next call tries again.
    fn record(&mut self, kind: &str, text: &str, at: Time, stderr: &mut impl Write) {
        let notice;
        let mut is_notice = false;
        let prefix = self.newline();
        let (text, counted) = if kind == "panic" {
            (text, false)
        } else if self.full {
            return;
        } else if self.written + prefix.len() + text.len() > MAX_RUN {
            is_notice = true;
            notice = entry("notice", NOTICE, at);
            (notice.as_str(), false)
        } else {
            (text, true)
        };
        let mut head = String::new();
        if self.file.is_none() {
            let (file, name) = match self.create() {
                Ok(created) => created,
                Err(err) => {
                    let _ = writeln!(stderr, "run log not written. {err}");
                    return;
                }
            };
            head = format!("{}\n", self.header);
            self.file = Some(file);
            match list_stale(&self.dir, &name) {
                Ok(names) => remove_all(&self.dir, &names, stderr),
                Err(err) => {
                    let _ = writeln!(stderr, "run log not pruned. {err}");
                }
            }
        }
        if let Some(file) = &mut self.file {
            // Count before the write: a failed write may leave part of the text in the file.
            self.written += prefix.len() + if counted { text.len() } else { 0 };
            match file.write_all(format!("{head}{prefix}{text}").as_bytes()) {
                Ok(()) => {
                    self.torn = false;
                    self.full |= is_notice;
                }
                Err(err) => {
                    self.torn = true;
                    let _ = writeln!(stderr, "run log not written. {err}");
                }
            }
        }
    }

    /// A newline that ends the half line a failed write may have left, else nothing.
    fn newline(&self) -> &'static str {
        if self.torn { "\n" } else { "" }
    }
}

/// The log of this run, set by [`install`].
static RUN: Mutex<Option<RunLog>> = Mutex::new(None);

/// Records `text` as an entry of `kind` in the log in `state`. An empty state
/// records nothing.
fn record_error(state: &Mutex<Option<RunLog>>, kind: &str, text: &str) {
    let at = now(SystemTime::now());
    let text = entry(kind, text, at);
    // A poisoned lock still holds a usable log.
    let mut guard = state.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(log) = guard.as_mut() {
        log.record(kind, &text, at, &mut io::stderr());
    }
}

/// Records a panic as the last entry of the log in `state`. A lock that is
/// held records nothing: waiting for it could hang the thread that panicked.
fn record_panic(state: &Mutex<Option<RunLog>>, thread: &str, info: &str, backtrace: Option<&str>) {
    let at = now(SystemTime::now());
    let text = entry("panic", &report(thread, info, backtrace), at);
    let Ok(mut guard) = state.try_lock() else {
        return;
    };
    if let Some(log) = guard.as_mut() {
        log.record("panic", &text, at, &mut io::stderr());
    }
}

/// Records an error of `kind` (`error` or `startup`) in the log of this run.
pub(crate) fn error(kind: &str, text: &str) {
    record_error(&RUN, kind, text);
}

/// Starts the log of this run in `dir`, named after `start_secs`, and installs
/// a panic hook that records the panic, then runs the hook installed before.
pub(crate) fn install(dir: PathBuf, start_secs: u64) {
    *RUN.lock().unwrap_or_else(PoisonError::into_inner) = Some(RunLog::new(dir, start_secs));
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        record_panic(
            &RUN,
            thread.name().unwrap_or("<unnamed>"),
            &info.to_string(),
            captured_backtrace().as_deref(),
        );
        previous(info);
    }));
}

/// The seconds since the epoch, for the start time the run passes to [`install`].
pub(crate) fn start_secs() -> u64 {
    now(SystemTime::now()).0
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::unnecessary_wraps)]

    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use rstest::rstest;

    use super::*;
    use crate::tests::{ScratchDir, names_of, old_names};

    /// 2001-02-03T04:05:06Z.
    const START: u64 = 981_173_106;

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    /// The file of a run that starts at `START`.
    fn file_of(dir: &Path) -> PathBuf {
        dir.join(format!("{}.log", file_stem(START)))
    }

    /// Records `body` as an entry of `kind` and returns what went to stderr.
    fn record(log: &mut RunLog, kind: &str, body: &str) -> String {
        let at = (START, 7);
        let mut stderr = Vec::new();
        log.record(kind, &entry(kind, body, at), at, &mut stderr);
        String::from_utf8(stderr).unwrap()
    }

    #[test]
    fn the_first_entry_creates_the_file_with_mode_0600() {
        let dir = ScratchDir::new("run-log-file-mode");
        let mut log = RunLog::new(dir.to_path_buf(), START);

        record(&mut log, "error", "boom");

        assert_eq!(mode(&file_of(&dir)), 0o600);
    }

    #[test]
    fn the_first_entry_creates_every_missing_directory_with_mode_0700() {
        let dir = ScratchDir::new("run-log-dir-mode");
        let outer = dir.join("a");
        let inner = outer.join("b");
        let mut log = RunLog::new(inner.clone(), START);

        record(&mut log, "error", "boom");

        assert_eq!((mode(&outer), mode(&inner)), (0o700, 0o700));
    }

    #[test]
    fn an_existing_directory_keeps_its_mode_when_the_first_entry_creates_the_file() {
        let dir = ScratchDir::new("run-log-dir-existing");
        let existing = dir.join("a");
        DirBuilder::new().mode(0o755).create(&existing).unwrap();
        fs::set_permissions(&existing, fs::Permissions::from_mode(0o755)).unwrap();
        let mut log = RunLog::new(existing.clone(), START);

        record(&mut log, "error", "boom");

        assert_eq!(mode(&existing), 0o755);
    }

    #[test]
    fn the_first_line_of_the_file_is_the_header() {
        let dir = ScratchDir::new("run-log-header");
        let mut log = RunLog::new(dir.to_path_buf(), START);

        record(&mut log, "error", "boom");

        let text = fs::read_to_string(file_of(&dir)).unwrap();
        let first = text.lines().next().unwrap();
        assert!(first.ends_with(concat!(" ", env!("CARGO_PKG_VERSION"))));
    }

    #[test]
    fn the_second_line_of_the_file_is_the_time_line_of_the_first_entry() {
        let dir = ScratchDir::new("run-log-header-entry");
        let mut log = RunLog::new(dir.to_path_buf(), START);

        record(&mut log, "error", "boom");

        let text = fs::read_to_string(file_of(&dir)).unwrap();
        assert_eq!(text.lines().nth(1), Some("2001-02-03T04:05:06.007Z error"));
    }

    #[test]
    fn a_new_log_touches_no_disk() {
        let dir = ScratchDir::new("run-log-lazy");
        let target = dir.join("logs");

        let _log = RunLog::new(target.clone(), START);

        assert!(!target.exists());
    }

    #[test]
    fn the_entries_of_one_run_go_to_one_file_in_order() {
        let dir = ScratchDir::new("run-log-order");
        let state = Mutex::new(Some(RunLog::new(dir.to_path_buf(), START)));

        record_error(&state, "error", "first");
        record_error(&state, "startup", "second");
        record_panic(&state, "main", "panicked at a.rs:1:2:\nboom", None);

        let names = names_of(&dir);
        assert_eq!(names.len(), 1);
        let text = fs::read_to_string(dir.join(&names[0])).unwrap();
        let positions = [
            "error\nfirst\n",
            "startup\nsecond\n",
            "panic\nthread 'main' panicked at a.rs:1:2:\nboom\n\n",
        ]
        .map(|part| text.find(part).unwrap());
        assert!(positions.is_sorted());
        assert!(text.ends_with("boom\n\n"));
    }

    #[test]
    fn two_runs_that_start_in_the_same_second_write_two_files() {
        let dir = ScratchDir::new("run-log-same-second");
        let mut first = RunLog::new(dir.to_path_buf(), START);
        let mut second = RunLog::new(dir.to_path_buf(), START);

        record(&mut first, "error", "one");
        record(&mut second, "error", "two");

        assert_eq!(
            names_of(&dir),
            [
                format!("{}.log", file_stem(START)),
                format!("{}.log", file_stem(START + 1))
            ]
        );
    }

    /// A log whose ten start-time names are all taken in `dir`.
    fn log_with_taken_names(dir: &Path) -> RunLog {
        for offset in 0..ATTEMPTS {
            fs::write(dir.join(format!("{}.log", file_stem(START + offset))), "").unwrap();
        }
        RunLog::new(dir.to_path_buf(), START)
    }

    #[test]
    fn ten_taken_names_print_the_io_error() {
        let dir = ScratchDir::new("run-log-taken");
        let mut log = log_with_taken_names(&dir);
        let err = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(file_of(&dir))
            .unwrap_err();

        let stderr = record(&mut log, "error", "boom");

        assert_eq!(stderr, format!("run log not written. {err}\n"));
    }

    #[test]
    fn the_tenth_start_time_is_tried() {
        let dir = ScratchDir::new("run-log-tenth");
        for offset in 0..ATTEMPTS - 1 {
            fs::write(dir.join(format!("{}.log", file_stem(START + offset))), "").unwrap();
        }
        let mut log = RunLog::new(dir.to_path_buf(), START);

        record(&mut log, "error", "boom");

        assert!(names_of(&dir).contains(&format!("{}.log", file_stem(START + 9))));
    }

    #[test]
    fn ten_taken_names_create_no_file() {
        let dir = ScratchDir::new("run-log-taken-none");
        let mut log = log_with_taken_names(&dir);

        record(&mut log, "error", "boom");

        assert_eq!(names_of(&dir).len(), 10);
        assert!(log.file.is_none());
    }

    /// Fills the run cap with one entry of `MAX_RUN - spare` bytes.
    fn fill(log: &mut RunLog, spare: usize) {
        let at = (START, 7);
        let text = "a".repeat(MAX_RUN - spare);
        log.record("error", &text, at, &mut Vec::new());
    }

    #[rstest]
    #[case::exactly_1_mib("bbbbbbbbbb", false)]
    #[case::one_byte_over("bbbbbbbbbbb", true)]
    fn the_entry_that_passes_1_mib_is_replaced_by_a_notice(
        #[case] last: &str,
        #[case] replaced: bool,
    ) {
        let dir = ScratchDir::new("run-log-cap");
        let mut log = RunLog::new(dir.to_path_buf(), START);
        fill(&mut log, 10);

        log.record("error", last, (START, 7), &mut Vec::new());

        let text = fs::read_to_string(file_of(&dir)).unwrap();
        assert_eq!(text.ends_with(last), !replaced);
        assert_eq!(
            text.ends_with("notice\nlog full. later errors are not recorded\n\n"),
            replaced
        );
    }

    /// A log whose notice is written and that has room left for a short entry.
    fn full_log(dir: &Path) -> RunLog {
        let mut log = RunLog::new(dir.to_path_buf(), START);
        fill(&mut log, 64);
        record(&mut log, "error", &"o".repeat(100));
        log
    }

    #[test]
    fn entries_after_the_notice_are_dropped() {
        let dir = ScratchDir::new("run-log-full");
        let mut log = full_log(&dir);

        record(&mut log, "startup", "zz");

        let text = fs::read_to_string(file_of(&dir)).unwrap();
        assert!(!text.contains("zz"));
    }

    #[test]
    fn the_notice_is_written_on_the_next_entry_when_its_creation_failed() {
        let dir = ScratchDir::new("run-log-notice-retry");
        let blocker = dir.join("blocker");
        fs::write(&blocker, "").unwrap();
        let target = blocker.join("logs");
        let mut log = RunLog::new(target.clone(), START);
        log.written = MAX_RUN;
        record(&mut log, "error", "first");
        fs::remove_file(&blocker).unwrap();

        record(&mut log, "error", "second");

        let text = fs::read_to_string(file_of(&target)).unwrap();
        assert!(text.ends_with(" notice\nlog full. later errors are not recorded\n\n"));
    }

    #[test]
    fn a_panic_is_written_after_the_cap_is_reached() {
        let dir = ScratchDir::new("run-log-full-panic");
        let mut log = full_log(&dir);

        record(&mut log, "panic", &"p".repeat(100));

        let text = fs::read_to_string(file_of(&dir)).unwrap();
        assert!(text.ends_with(&format!(" panic\n{}\n\n", "p".repeat(100))));
    }

    #[test]
    fn a_directory_that_cannot_be_created_prints_the_io_error() {
        let dir = ScratchDir::new("run-log-blocked");
        fs::write(dir.join("file"), "").unwrap();
        let blocked = dir.join("file").join("logs");
        let err = DirBuilder::new()
            .recursive(true)
            .create(&blocked)
            .unwrap_err();
        let mut log = RunLog::new(blocked, START);

        let stderr = record(&mut log, "error", "boom");

        assert_eq!(stderr, format!("run log not written. {err}\n"));
    }

    #[test]
    fn the_next_entry_after_a_failed_creation_tries_again() {
        let dir = ScratchDir::new("run-log-retry");
        let blocker = dir.join("blocker");
        fs::write(&blocker, "").unwrap();
        let target = blocker.join("logs");
        let mut log = RunLog::new(target.clone(), START);
        record(&mut log, "error", "first");
        fs::remove_file(&blocker).unwrap();

        let stderr = record(&mut log, "error", "second");

        assert_eq!(stderr, "");
        assert!(
            fs::read_to_string(file_of(&target))
                .unwrap()
                .contains("second")
        );
    }

    #[test]
    fn an_error_is_recorded_when_the_lock_is_poisoned() {
        let dir = ScratchDir::new("run-log-poisoned");
        let state = Mutex::new(Some(RunLog::new(dir.to_path_buf(), START)));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = state.lock().unwrap();
            panic!("poison");
        }));

        record_error(&state, "error", "boom");

        let names = names_of(&dir);
        assert!(
            fs::read_to_string(dir.join(&names[0]))
                .unwrap()
                .contains("boom")
        );
    }

    #[test]
    fn a_panic_with_the_lock_held_writes_no_file() {
        let dir = ScratchDir::new("run-log-held");
        let state = Mutex::new(Some(RunLog::new(dir.to_path_buf(), START)));
        let _guard = state.lock().unwrap();

        record_panic(&state, "main", "panicked", None);

        assert_eq!(names_of(&dir), Vec::<String>::new());
    }

    #[test]
    fn an_error_with_an_empty_state_is_ignored() {
        let state = Mutex::new(None);

        record_error(&state, "error", "boom");

        assert!(state.lock().unwrap().is_none());
    }

    #[test]
    fn the_first_entry_leaves_the_ten_newest_files() {
        let dir = ScratchDir::new("run-log-first-prune");
        for name in old_names(12) {
            fs::write(dir.join(name), "").unwrap();
        }
        let mut log = RunLog::new(dir.to_path_buf(), START);

        record(&mut log, "error", "boom");

        let mut expected = old_names(12)[3..].to_vec();
        expected.push(format!("{}.log", file_stem(START)));
        assert_eq!(names_of(&dir), expected);
    }

    /// Records one entry in a directory with mode 0300, which can be written and
    /// searched but not read. Returns the directory and what went to stderr, or
    /// `None` when the directory can be read anyway, as it can for root.
    fn record_in_unreadable_dir(name: &str) -> Option<(ScratchDir, String)> {
        let dir = ScratchDir::new(name);
        fs::set_permissions(&*dir, fs::Permissions::from_mode(0o300)).unwrap();
        let readable = fs::read_dir(&*dir).is_ok();
        let stderr = (!readable)
            .then(|| record(&mut RunLog::new(dir.to_path_buf(), START), "error", "boom"));
        fs::set_permissions(&*dir, fs::Permissions::from_mode(0o700)).unwrap();
        stderr.map(|stderr| (dir, stderr))
    }

    #[test]
    fn an_unreadable_directory_prints_one_pruning_line() {
        let Some((_dir, stderr)) = record_in_unreadable_dir("run-log-unreadable-line") else {
            return;
        };

        assert!(stderr.starts_with("run log not pruned. "));
        assert_eq!(stderr.lines().count(), 1);
    }

    #[test]
    fn an_unreadable_directory_still_gets_the_entry() {
        let Some((dir, _stderr)) = record_in_unreadable_dir("run-log-unreadable-entry") else {
            return;
        };

        assert!(fs::read_to_string(file_of(&dir)).unwrap().contains("boom"));
    }

    /// A log in `dir` whose file is open for reading only, so a write fails.
    fn log_with_read_only_file(dir: &Path) -> RunLog {
        let existing = dir.join("existing");
        fs::write(&existing, "").unwrap();
        let mut log = RunLog::new(dir.to_path_buf(), START);
        log.file = Some(File::open(&existing).unwrap());
        log
    }

    #[test]
    fn a_failed_write_prints_the_io_error() {
        let dir = ScratchDir::new("run-log-write-fails");
        let mut log = log_with_read_only_file(&dir);

        let stderr = record(&mut log, "error", "boom");

        assert!(stderr.starts_with("run log not written. "));
    }

    /// Records `first` into a log whose write fails, then gives the log a
    /// writable file and returns it with the path of that file.
    fn log_after_a_failed_write(dir: &Path, first: &str) -> (RunLog, PathBuf) {
        let mut log = log_with_read_only_file(dir);
        record(&mut log, "error", first);
        let path = dir.join("existing");
        log.file = Some(OpenOptions::new().append(true).open(&path).unwrap());
        (log, path)
    }

    #[test]
    fn the_entry_after_a_failed_write_begins_with_a_newline() {
        let dir = ScratchDir::new("run-log-torn");
        let (mut log, path) = log_after_a_failed_write(&dir, "first");

        record(&mut log, "error", "second");

        assert_eq!(
            fs::read_to_string(path).unwrap(),
            format!("\n{}", entry("error", "second", (START, 7)))
        );
    }

    #[test]
    fn the_newline_after_a_failed_write_is_written_once() {
        let dir = ScratchDir::new("run-log-torn-once");
        let (mut log, path) = log_after_a_failed_write(&dir, "first");
        record(&mut log, "error", "second");

        record(&mut log, "error", "third");

        let [second, third] = ["second", "third"].map(|body| entry("error", body, (START, 7)));
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            format!("\n{second}{third}")
        );
    }

    #[test]
    fn the_newline_after_a_failed_write_counts_toward_the_run_cap() {
        let dir = ScratchDir::new("run-log-torn-count");
        let (mut log, _path) = log_after_a_failed_write(&dir, "first");

        record(&mut log, "error", "second");

        let sizes = ["first", "second"].map(|body| entry("error", body, (START, 7)).len());
        assert_eq!(log.written, sizes[0] + 1 + sizes[1]);
    }

    #[test]
    fn a_failed_write_of_the_notice_leaves_the_log_not_full() {
        let dir = ScratchDir::new("run-log-write-fails-notice");
        let mut log = log_with_read_only_file(&dir);
        log.written = MAX_RUN;

        record(&mut log, "error", "boom");

        assert!(!log.full);
    }

    #[rstest]
    #[case::exactly_1_mib(0, false)]
    #[case::one_byte_over(1, true)]
    fn the_cap_check_after_a_failed_write_counts_its_newline(
        #[case] extra: usize,
        #[case] replaced: bool,
    ) {
        let dir = ScratchDir::new("run-log-torn-cap");
        let (mut log, path) = log_after_a_failed_write(&dir, "first");
        let first = entry("error", "first", (START, 7)).len();
        let text = "b".repeat(MAX_RUN - first - 1 + extra);

        log.record("error", &text, (START, 7), &mut Vec::new());

        let file = fs::read_to_string(path).unwrap();
        assert_eq!(file.ends_with(&text), !replaced);
        assert_eq!(file.contains(" notice\n"), replaced);
    }

    #[test]
    fn a_file_that_cannot_be_created_prints_the_io_error() {
        let dir = ScratchDir::new("run-log-no-create");
        fs::set_permissions(&*dir, fs::Permissions::from_mode(0o500)).unwrap();
        let open = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(file_of(&dir));
        let Err(err) = open else {
            // The directory can be written anyway, as it can for root.
            fs::set_permissions(&*dir, fs::Permissions::from_mode(0o700)).unwrap();
            return;
        };
        let mut log = RunLog::new(dir.to_path_buf(), START);

        let stderr = record(&mut log, "error", "boom");
        fs::set_permissions(&*dir, fs::Permissions::from_mode(0o700)).unwrap();

        assert_eq!(stderr, format!("run log not written. {err}\n"));
    }
}
