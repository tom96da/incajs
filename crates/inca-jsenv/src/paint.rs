// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! ANSI colors for terminal output.

/// A color or weight, with the codes that open and close it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Style {
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Bold,
    Grey,
}

impl Style {
    fn codes(self) -> (u8, u8) {
        match self {
            Style::Red => (31, 39),
            Style::Green => (32, 39),
            Style::Yellow => (33, 39),
            Style::Blue => (34, 39),
            Style::Magenta => (35, 39),
            Style::Cyan => (36, 39),
            Style::Bold => (1, 22),
            Style::Grey => (90, 39),
        }
    }
}

/// `text` in `style` when `on`, unchanged otherwise. Each non-empty line is
/// opened and closed on its own.
pub(crate) fn paint(on: bool, style: Style, text: &str) -> String {
    if !on {
        return text.to_owned();
    }
    let (open, close) = style.codes();
    text.split('\n')
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("\x1b[{open}m{line}\x1b[{close}m")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::red(Style::Red, 31, 39)]
    #[case::green(Style::Green, 32, 39)]
    #[case::yellow(Style::Yellow, 33, 39)]
    #[case::blue(Style::Blue, 34, 39)]
    #[case::magenta(Style::Magenta, 35, 39)]
    #[case::cyan(Style::Cyan, 36, 39)]
    #[case::bold(Style::Bold, 1, 22)]
    #[case::grey(Style::Grey, 90, 39)]
    fn each_style_opens_and_closes_with_its_own_codes(
        #[case] style: Style,
        #[case] open: u8,
        #[case] close: u8,
    ) {
        assert_eq!(
            paint(true, style, "x"),
            format!("\x1b[{open}mx\x1b[{close}m")
        );
    }

    #[test]
    fn every_line_is_opened_and_closed_on_its_own() {
        assert_eq!(
            paint(true, Style::Green, "a\nb"),
            "\x1b[32ma\x1b[39m\n\x1b[32mb\x1b[39m"
        );
        assert_eq!(
            paint(true, Style::Green, "a\n\nb\n"),
            "\x1b[32ma\x1b[39m\n\n\x1b[32mb\x1b[39m\n"
        );
        assert_eq!(paint(false, Style::Green, "a\nb"), "a\nb");
    }
}
