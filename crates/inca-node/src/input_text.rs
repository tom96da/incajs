// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The filters that turn typed and pasted text into input text.

/// `text` without the chars for which [`char::is_control`] holds.
#[must_use]
pub fn typed(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

/// `text` without CR and LF.
#[must_use]
pub fn sanitized(text: &str) -> String {
    text.chars().filter(|c| !matches!(c, '\r' | '\n')).collect()
}

/// `text` with each CRLF, then each remaining CR and LF, replaced by one
/// space.
#[must_use]
pub fn pasted(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn typed_text_drops_control_characters() {
        assert_eq!(typed("a\tb\nc\u{8}"), "abc");
    }

    #[test]
    fn sanitized_text_drops_line_breaks_only() {
        assert_eq!(sanitized("a\r\nb\tc"), "ab\tc");
    }

    #[rstest]
    #[case::lf("a\nb\tc", "a b\tc")]
    #[case::crlf("a\r\nb", "a b")]
    #[case::cr("a\rb", "a b")]
    fn pasted_text_turns_each_line_break_into_a_space(#[case] text: &str, #[case] want: &str) {
        assert_eq!(pasted(text), want);
    }
}
