// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The text of one entry and the UTC time it carries.

use std::backtrace::{Backtrace, BacktraceStatus};
use std::time::{SystemTime, UNIX_EPOCH};

/// The most bytes of one entry body.
const MAX_BODY: usize = 64 * 1024;
/// The last second the file name can hold in four digits of year.
const MAX_SECS: u64 = 253_402_300_799;

/// Seconds and milliseconds since the Unix epoch.
pub(super) type Time = (u64, u32);

/// The seconds and milliseconds since the epoch of `time`, or `(0, 0)` for a
/// time before it.
pub(super) fn now(time: SystemTime) -> Time {
    time.duration_since(UNIX_EPOCH)
        .map_or((0, 0), |since| (since.as_secs(), since.subsec_millis()))
}

/// The UTC date and time of `secs` as year, month, day, hour, minute and second.
/// Seconds past the year 9999 count as its last second.
pub(super) fn civil(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let secs = secs.min(MAX_SECS);
    // Days since 0000-03-01, so the leap day is the last day of a year.
    let days = secs / 86_400 + 719_468;
    let era = days / 146_097;
    let day_of_era = days % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    let time = secs % 86_400;
    (year, month, day, time / 3600, time % 3600 / 60, time % 60)
}

/// The time line of an entry: `YYYY-MM-DDTHH:MM:SS.mmmZ`.
fn stamp(secs: u64, millis: u32) -> String {
    let (year, month, day, hour, minute, second) = civil(secs);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// The first line of a file: `package` and `version` when the build names a
/// package, else the binary `name`, `os`, `arch` and `version`.
pub(super) fn header(
    package: Option<&str>,
    name: &str,
    version: &str,
    os: &str,
    arch: &str,
) -> String {
    match package.filter(|package| !package.is_empty()) {
        Some(package) => format!("{package} {version}"),
        None => format!("{name} {os}/{arch} {version}"),
    }
}

/// The body of a panic entry: the thread and the panic, then the backtrace
/// when there is one.
pub(super) fn report(thread: &str, info: &str, backtrace: Option<&str>) -> String {
    let mut text = format!("thread '{thread}' {info}\n");
    if let Some(backtrace) = backtrace {
        text.push_str("stack backtrace:\n");
        text.push_str(backtrace);
        text.push('\n');
    }
    text
}

/// The text of one entry: the time and `kind`, the `body` cut to [`MAX_BODY`]
/// bytes, and a blank line.
pub(super) fn entry(kind: &str, body: &str, (secs, millis): Time) -> String {
    let body = body.trim_end_matches('\n');
    let mut end = body.len().min(MAX_BODY);
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    let mut text = format!("{} {kind}\n{}\n", stamp(secs, millis), &body[..end]);
    if end < body.len() {
        text.push_str("[cut at 64 KiB]\n");
    }
    text.push('\n');
    text
}

/// The backtrace of the panic that runs the hook, or `None` when the
/// environment asks for none.
pub(super) fn captured_backtrace() -> Option<String> {
    let backtrace = Backtrace::capture();
    (backtrace.status() == BacktraceStatus::Captured).then(|| backtrace.to_string())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rstest::rstest;

    use super::*;

    /// 2001-02-03T04:05:06Z.
    const START: u64 = 981_173_106;

    #[rstest]
    #[case::epoch(0, (1970, 1, 1, 0, 0, 0))]
    #[case::leap_day_of_a_400_year(951_782_400, (2000, 2, 29, 0, 0, 0))]
    #[case::leap_day_of_a_4_year(1_709_164_800, (2024, 2, 29, 0, 0, 0))]
    #[case::end_of_a_leap_year(1_735_689_599, (2024, 12, 31, 23, 59, 59))]
    #[case::start_of_the_next_year(1_735_689_600, (2025, 1, 1, 0, 0, 0))]
    #[case::last_day_of_february_in_a_100_year(4_107_542_399, (2100, 2, 28, 23, 59, 59))]
    #[case::first_day_of_march_in_a_100_year(4_107_542_400, (2100, 3, 1, 0, 0, 0))]
    #[case::afternoon(1_793_637_000, (2026, 11, 2, 16, 30, 0))]
    #[case::last_second_of_year_9999(253_402_300_799, (9999, 12, 31, 23, 59, 59))]
    #[case::past_year_9999_saturates(253_402_300_800, (9999, 12, 31, 23, 59, 59))]
    fn civil_converts_seconds_to_a_utc_date_and_time(
        #[case] secs: u64,
        #[case] expected: (u64, u64, u64, u64, u64, u64),
    ) {
        assert_eq!(civil(secs), expected);
    }

    #[test]
    fn a_stamp_pads_the_milliseconds_to_three_digits() {
        assert_eq!(stamp(START, 7), "2001-02-03T04:05:06.007Z");
    }

    #[test]
    fn a_time_before_1970_is_second_zero() {
        assert_eq!(now(UNIX_EPOCH - Duration::from_secs(1)), (0, 0));
    }

    #[test]
    fn a_time_splits_into_seconds_and_milliseconds() {
        assert_eq!(now(UNIX_EPOCH + Duration::from_millis(1234)), (1, 234));
    }

    #[rstest]
    #[case::package(Some("@incajs/host-linux-x64"), "@incajs/host-linux-x64 0.0.28")]
    #[case::no_package(None, "inca-host linux/x86_64 0.0.28")]
    #[case::empty_package(Some(""), "inca-host linux/x86_64 0.0.28")]
    fn the_header_names_the_package_or_the_binary(
        #[case] package: Option<&str>,
        #[case] expected: &str,
    ) {
        assert_eq!(
            header(package, "inca-host", "0.0.28", "linux", "x86_64"),
            expected
        );
    }

    #[rstest]
    #[case::no_trailing_newline("boom")]
    #[case::several_trailing_newlines("boom\n\n\n")]
    fn an_entry_is_the_time_and_kind_the_body_and_one_blank_line(#[case] body: &str) {
        assert_eq!(
            entry("error", body, (START, 7)),
            "2001-02-03T04:05:06.007Z error\nboom\n\n"
        );
    }

    #[rstest]
    #[case::without_a_backtrace(None, "thread 'main' panicked at a.rs:1:2:\nboom\n")]
    #[case::with_a_backtrace(
        Some("0: f"),
        "thread 'main' panicked at a.rs:1:2:\nboom\nstack backtrace:\n0: f\n"
    )]
    fn a_report_names_the_thread_and_the_panic(
        #[case] backtrace: Option<&str>,
        #[case] expected: &str,
    ) {
        assert_eq!(
            report("main", "panicked at a.rs:1:2:\nboom", backtrace),
            expected
        );
    }

    #[rstest]
    #[case::exactly_64_kib(MAX_BODY, "")]
    #[case::one_byte_over(MAX_BODY + 1, "[cut at 64 KiB]\n")]
    fn the_cut_marker_follows_a_body_over_64_kib(#[case] len: usize, #[case] marker: &str) {
        let text = entry("error", &"a".repeat(len), (START, 7));

        assert_eq!(
            text,
            format!(
                "2001-02-03T04:05:06.007Z error\n{}\n{marker}\n",
                "a".repeat(MAX_BODY)
            )
        );
    }

    #[test]
    fn a_body_of_multi_byte_characters_is_cut_at_a_character_boundary() {
        let text = entry("error", &"\u{20ac}".repeat(30_000), (START, 7));

        let kept = "\u{20ac}".repeat(21_845);
        assert!(text.ends_with(&format!("{kept}\n[cut at 64 KiB]\n\n")));
        assert!(!text.contains(&"\u{20ac}".repeat(21_846)));
    }
}
