// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::parse::decimal;
use super::spec::ColorSpec;
use crate::tree::AttributeValue;

/// A number from `0` to `0xffffff` (opaque), or a string holding `#rgb`,
/// `#rgba`, `#rrggbb`, `#rrggbbaa`, `transparent`, `rgb(...)` or `rgba(...)`
/// with optional surrounding ASCII whitespace. Any other value is ignored.
pub(super) fn as_color(value: &AttributeValue) -> Option<ColorSpec> {
    match value {
        AttributeValue::Number(n) if (0.0..=f64::from(0x00FF_FFFF_u32)).contains(n) => {
            // The range check above keeps the cast inside `u32`.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            Some(ColorSpec::opaque(*n as u32))
        }
        AttributeValue::String(s) => parse_color(s.trim_ascii()),
        _ => None,
    }
}

fn parse_color(s: &str) -> Option<ColorSpec> {
    if let Some(hex) = s.strip_prefix('#') {
        return parse_hex(hex);
    }
    if s.eq_ignore_ascii_case("transparent") {
        return Some(ColorSpec { rgb: 0, alpha: 0 });
    }
    let args = function_args(s, "rgba").or_else(|| function_args(s, "rgb"))?;
    parse_rgb(args)
}

/// The text between the parentheses of `name(...)`, with `name` in any ASCII
/// case and `)` last. `get` returns `None` inside a multibyte character.
fn function_args<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let head = s.get(..name.len())?;
    if !head.eq_ignore_ascii_case(name) {
        return None;
    }
    s.get(name.len()..)?.strip_prefix('(')?.strip_suffix(')')
}

/// `hex` is the text after `#`: 3, 4, 6 or 8 ASCII hex digits.
fn parse_hex(hex: &str) -> Option<ColorSpec> {
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let full: String = match hex.len() {
        3 | 4 => hex.chars().flat_map(|c| [c, c]).collect(),
        6 | 8 => hex.to_owned(),
        _ => return None,
    };
    let value = u32::from_str_radix(&full, 16).ok()?;
    if full.len() == 6 {
        return Some(ColorSpec::opaque(value));
    }
    let [r, g, b, alpha] = value.to_be_bytes();
    Some(ColorSpec {
        rgb: u32::from_be_bytes([0, r, g, b]),
        alpha,
    })
}

/// One channel or alpha token.
#[derive(Clone, Copy)]
enum Component {
    Number(f64),
    Percent(f64),
}

impl Component {
    fn is_percent(self) -> bool {
        matches!(self, Self::Percent(_))
    }

    fn channel(self) -> u8 {
        to_byte(match self {
            Self::Number(n) => n,
            Self::Percent(p) => p * 255.0 / 100.0,
        })
    }

    fn alpha(self) -> u8 {
        to_byte(match self {
            Self::Number(a) => a * 255.0,
            Self::Percent(p) => p * 255.0 / 100.0,
        })
    }
}

/// Rounds half up. `as` saturates, so a value below `0` is `0` and a value
/// above `255` (infinity included) is `255`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_byte(n: f64) -> u8 {
    n.round() as u8
}

/// A CSS number: `decimal`, and every `.` is followed by an ASCII digit.
fn css_number(s: &str) -> Option<f64> {
    if s.as_bytes()
        .windows(2)
        .any(|pair| pair[0] == b'.' && !pair[1].is_ascii_digit())
    {
        return None;
    }
    decimal(s)
}

/// A number, or a number followed by `%`.
fn component(token: &str) -> Option<Component> {
    match token.strip_suffix('%') {
        Some(n) => css_number(n).map(Component::Percent),
        None => css_number(token).map(Component::Number),
    }
}

/// `args` is the text between the parentheses of `rgb(` or `rgba(`.
fn parse_rgb(args: &str) -> Option<ColorSpec> {
    let legacy = args.contains(',');
    let (channels, alpha): (Vec<&str>, Option<&str>) = if legacy {
        let mut parts: Vec<&str> = args.split(',').map(str::trim_ascii).collect();
        let alpha = match parts.len() {
            3 => None,
            4 => parts.pop(),
            _ => return None,
        };
        (parts, alpha)
    } else {
        let (head, alpha) = match args.split_once('/') {
            Some((head, alpha)) => (head, Some(alpha.trim_ascii())),
            None => (args, None),
        };
        (head.split_ascii_whitespace().collect(), alpha)
    };
    let [r, g, b] = channels.as_slice() else {
        return None;
    };
    let (r, g, b) = (component(r)?, component(g)?, component(b)?);
    let kinds = [r, g, b].map(Component::is_percent);
    if legacy && kinds.iter().any(|kind| *kind != kinds[0]) {
        return None;
    }
    let alpha = match alpha {
        Some(token) => component(token)?.alpha(),
        None => 255,
    };
    Some(ColorSpec {
        rgb: u32::from_be_bytes([0, r.channel(), g.channel(), b.channel()]),
        alpha,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::element::spec::build_spec;
    use crate::element::tests::{num, style_and_warnings, text};
    use crate::tree::VirtualTree;

    const INVALID: &str = "ignoring invalid color for style key `background`";

    /// The `background` of `value`, and the warnings setting it prints.
    fn read(value: &str) -> (Option<ColorSpec>, Vec<String>) {
        let (style, warnings) = style_and_warnings(&[("background", text(value))]);
        (style.background, warnings)
    }

    #[test]
    fn color_accepts_hex_strings_as_well_as_numbers() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        tree.set_style(id, "background", "#505050").unwrap();
        tree.set_style(id, "border_color", "#00f").unwrap();

        let spec = build_spec(&tree, id).unwrap();
        assert_eq!(spec.style.background, Some(ColorSpec::opaque(0x505050)));
        assert_eq!(
            spec.style.border_color,
            Some(ColorSpec::opaque(0x0000ff)),
            "a 3-digit hex string must expand each digit, not zero-pad it"
        );
    }

    #[test]
    fn malformed_color_string_is_ignored_with_a_warning() {
        let (style, warnings) = style_and_warnings(&[("background", text("not-a-color"))]);
        assert_eq!(style.background, None);
        assert_eq!(
            warnings,
            ["ignoring invalid color for style key `background`"]
        );
    }

    #[rstest]
    #[case::plus_six_digits("#+abcde")]
    #[case::minus_six_digits("#-abcde")]
    #[case::plus_three_digits("#+ab")]
    #[case::plus_four_digits("#+abcd")]
    #[case::non_hex_digit("#12345g")]
    #[case::non_hex_short("#gg0")]
    #[case::two_digits("#12")]
    #[case::seven_digits("#1234567")]
    #[case::no_hash("ffffff")]
    #[case::hash_only("#")]
    #[case::multibyte("#ééé")]
    fn a_color_string_with_a_sign_or_non_hex_digit_is_ignored(#[case] bad: &str) {
        let (style, warnings) = style_and_warnings(&[("text_color", text(bad))]);
        assert_eq!(style.text_color, None, "{bad}");
        assert_eq!(warnings.len(), 1, "{bad}");
    }

    #[test]
    fn color_strings_accept_either_hex_case() {
        let (style, warnings) = style_and_warnings(&[
            ("background", text("#AbCdEf")),
            ("border_color", text("#FfF")),
        ]);
        assert_eq!(style.background, Some(ColorSpec::opaque(0x00ab_cdef)));
        assert_eq!(style.border_color, Some(ColorSpec::opaque(0x00ff_ffff)));
        assert!(warnings.is_empty());
    }

    #[rstest]
    #[case::zero(0.0, 0)]
    #[case::max(f64::from(0xFF_FFFF), 0xFF_FFFF)]
    #[case::byte_max(255.0, 255)]
    fn numeric_colors_cover_exactly_zero_to_ffffff(#[case] n: f64, #[case] expected: u32) {
        let (style, warnings) = style_and_warnings(&[("background", num(n))]);
        assert_eq!(style.background, Some(ColorSpec::opaque(expected)), "{n}");
        assert!(warnings.is_empty(), "{n}");
    }

    #[rstest]
    #[case::half(0.5, 0)]
    #[case::one_and_half(1.5, 1)]
    #[case::near_max(16_777_214.5, 0xFF_FFFE)]
    fn a_fractional_numeric_color_rounds_toward_zero(#[case] n: f64, #[case] expected: u32) {
        let (style, warnings) = style_and_warnings(&[("background", num(n))]);
        assert_eq!(style.background, Some(ColorSpec::opaque(expected)), "{n}");
        assert!(warnings.is_empty(), "{n}");
    }

    #[test]
    fn a_negative_fractional_numeric_color_is_ignored_with_a_warning() {
        let (style, warnings) = style_and_warnings(&[("background", num(-0.5))]);
        assert_eq!(style.background, None);
        assert_eq!(warnings.len(), 1);
    }

    #[rstest]
    #[case::negative(-1.0)]
    #[case::just_above_ffffff(f64::from(0x0100_0000))]
    #[case::u32_max(f64::from(u32::MAX))]
    #[case::huge(1e20)]
    #[case::nan(f64::NAN)]
    #[case::infinity(f64::INFINITY)]
    #[case::negative_infinity(f64::NEG_INFINITY)]
    fn an_out_of_range_numeric_color_is_ignored_with_a_warning(#[case] bad: f64) {
        let (style, warnings) = style_and_warnings(&[
            ("background", num(bad)),
            ("border_color", num(bad)),
            ("text_color", num(bad)),
        ]);
        assert_eq!(
            (style.background, style.border_color, style.text_color),
            (None, None, None),
            "{bad}"
        );
        assert_eq!(warnings.len(), 3, "{bad}");
    }

    #[test]
    fn a_bool_color_is_ignored_with_a_warning() {
        let (style, warnings) = style_and_warnings(&[("background", AttributeValue::Bool(true))]);
        assert_eq!(style.background, None);
        assert_eq!(warnings.len(), 1);
    }

    #[rstest]
    #[case::four_digits("#0f08", 0x00ff00, 136)]
    #[case::eight_digits("#00ff0080", 0x00ff00, 128)]
    fn a_hex_string_reads_its_channels_and_alpha(
        #[case] written: &str,
        #[case] rgb: u32,
        #[case] alpha: u8,
    ) {
        assert_eq!(
            read(written),
            (Some(ColorSpec { rgb, alpha }), vec![]),
            "{written}"
        );
    }

    #[rstest]
    #[case::lowercase("transparent", 0x000000, 0)]
    #[case::capitalized("Transparent", 0x000000, 0)]
    fn transparent_is_black_with_no_alpha(
        #[case] written: &str,
        #[case] rgb: u32,
        #[case] alpha: u8,
    ) {
        assert_eq!(
            read(written),
            (Some(ColorSpec { rgb, alpha }), vec![]),
            "{written}"
        );
    }

    #[rstest]
    #[case::hex(" #fff", 0xffffff, 255)]
    #[case::function("\n rgb(1,2,3)\t", 0x010203, 255)]
    fn a_color_string_ignores_surrounding_ascii_whitespace(
        #[case] written: &str,
        #[case] rgb: u32,
        #[case] alpha: u8,
    ) {
        assert_eq!(
            read(written),
            (Some(ColorSpec { rgb, alpha }), vec![]),
            "{written}"
        );
    }

    #[rstest]
    #[case::comma("rgb(1,2,3)", 0x010203, 255)]
    #[case::rgba_name("rgba(1,2,3)", 0x010203, 255)]
    #[case::upper_name("RGB(1,2,3)", 0x010203, 255)]
    #[case::spaced_commas("rgb( 1 , 2 , 3 )", 0x010203, 255)]
    #[case::space_syntax("rgb(1\t2\n3)", 0x010203, 255)]
    #[case::comma_alpha("rgb(1,2,3,0.5)", 0x010203, 128)]
    #[case::slash_alpha("rgb(1 2 3 / 0.5)", 0x010203, 128)]
    #[case::tight_slash_alpha("rgb(1 2 3/.5)", 0x010203, 128)]
    fn an_rgb_function_reads_either_syntax(
        #[case] written: &str,
        #[case] rgb: u32,
        #[case] alpha: u8,
    ) {
        assert_eq!(
            read(written),
            (Some(ColorSpec { rgb, alpha }), vec![]),
            "{written}"
        );
    }

    #[rstest]
    #[case::comma("rgb(10%,20%,30%)", 0x1a334d, 255)]
    #[case::space_mixed("rgb(10 20% 30)", 0x0a331e, 255)]
    fn an_rgb_function_reads_percent_channels(
        #[case] written: &str,
        #[case] rgb: u32,
        #[case] alpha: u8,
    ) {
        assert_eq!(
            read(written),
            (Some(ColorSpec { rgb, alpha }), vec![]),
            "{written}"
        );
    }

    #[rstest]
    #[case::number("rgb(1.5,2.5,3.5)", 0x020304, 255)]
    #[case::percent("rgb(50%,0%,0%)", 0x800000, 255)]
    fn a_channel_rounds_half_up(#[case] written: &str, #[case] rgb: u32, #[case] alpha: u8) {
        assert_eq!(
            read(written),
            (Some(ColorSpec { rgb, alpha }), vec![]),
            "{written}"
        );
    }

    #[rstest]
    #[case::finite("rgb(-1,300,0)", 0x00ff00, 255)]
    #[case::infinite("rgb(1e999,-1e999,0)", 0xff0000, 255)]
    fn a_channel_saturates(#[case] written: &str, #[case] rgb: u32, #[case] alpha: u8) {
        assert_eq!(
            read(written),
            (Some(ColorSpec { rgb, alpha }), vec![]),
            "{written}"
        );
    }

    #[rstest]
    #[case::half("rgb(0,0,0,0.5)", 0x000000, 128)]
    #[case::below_half("rgb(0,0,0,.499)", 0x000000, 127)]
    #[case::percent("rgb(0,0,0,5%)", 0x000000, 13)]
    fn an_alpha_rounds_half_up(#[case] written: &str, #[case] rgb: u32, #[case] alpha: u8) {
        assert_eq!(
            read(written),
            (Some(ColorSpec { rgb, alpha }), vec![]),
            "{written}"
        );
    }

    #[rstest]
    #[case::above_one("rgb(0,0,0,2)", 0x000000, 255)]
    #[case::negative("rgb(0,0,0,-1)", 0x000000, 0)]
    fn an_alpha_saturates(#[case] written: &str, #[case] rgb: u32, #[case] alpha: u8) {
        assert_eq!(
            read(written),
            (Some(ColorSpec { rgb, alpha }), vec![]),
            "{written}"
        );
    }

    #[rstest]
    #[case::too_few_parts("rgb(1,2)")]
    #[case::too_many_parts("rgb(1,2,3,4,5)")]
    #[case::no_channel("rgb()")]
    #[case::empty_alpha_part("rgb(1,2,3,)")]
    #[case::four_tokens("rgb(1 2 3 0.5)")]
    #[case::empty_alpha_after_slash("rgb(1 2 3 /)")]
    #[case::second_slash("rgb(1 2 3 / 0.5 / 0.2)")]
    #[case::slash_in_comma_syntax("rgb(1, 2, 3 / 0.5)")]
    #[case::space_before_paren("rgb (1,2,3)")]
    #[case::no_closing_paren("rgb(1,2,3")]
    #[case::text_after_paren("rgb(1,2,3)x")]
    #[case::non_ascii_space("rgb(1,2,3\u{a0})")]
    fn an_rgb_function_of_a_bad_shape_is_ignored_with_a_warning(#[case] written: &str) {
        assert_eq!(read(written), (None, vec![INVALID.to_owned()]), "{written}");
    }

    #[rstest]
    #[case::first_percent("rgb(10%,20,30)")]
    #[case::second_percent("rgb(10,20%,30)")]
    #[case::third_percent("rgb(10,20,30%)")]
    fn the_comma_syntax_does_not_mix_numbers_and_percents(#[case] written: &str) {
        assert_eq!(read(written), (None, vec![INVALID.to_owned()]), "{written}");
    }

    #[rstest]
    #[case::unit("rgb(1px,2,3)")]
    #[case::hex("rgb(0x1,2,3)")]
    #[case::dot_without_digit("rgb(1.e2,0,0)")]
    #[case::trailing_dot("rgb(1.,2,3)")]
    #[case::word("rgb(Infinity,0,0)")]
    #[case::bare_percent("rgb(%,0,0)")]
    #[case::none("rgb(none 0 0)")]
    #[case::calc("rgb(calc(1 + 1) 2 3)")]
    fn a_channel_that_is_not_a_css_number_is_ignored(#[case] written: &str) {
        assert_eq!(read(written), (None, vec![INVALID.to_owned()]), "{written}");
    }

    #[rstest]
    #[case::currentcolor("currentcolor")]
    #[case::hsl("hsl(0,0%,0%)")]
    #[case::unspaced_percents("rgb(10%20%30%)")]
    #[case::longer_name("rgbx(1,2,3)")]
    #[case::cut_character("rgé(1,2,3)")]
    #[case::only_whitespace("   ")]
    fn an_unsupported_notation_is_ignored_with_a_warning(#[case] written: &str) {
        assert_eq!(read(written), (None, vec![INVALID.to_owned()]), "{written}");
    }

    #[test]
    fn an_empty_color_string_is_ignored_quietly() {
        assert_eq!(read(""), (None, vec![]));
    }
}
