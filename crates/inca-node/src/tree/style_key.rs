// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The style keys a node's style props are stored under, and the keyword values they take.

use std::borrow::Cow;

/// Defines a keyword enum: its variants, `NAMES` and `from_name`. A variant
/// lists every name it takes, separated by `|`.
macro_rules! keywords {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $($(#[$vmeta:meta])* $variant:ident = $first:literal $(| $more:literal)*,)*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub enum $name {
            $($(#[$vmeta])* $variant,)*
        }

        impl $name {
            /// Every name a style value of this kind takes, in declaration order.
            pub const NAMES: &[&str] = &[$($first, $($more,)*)*];

            /// The variant `name` stands for. Names compare without ASCII case.
            #[must_use]
            pub fn from_name(name: &str) -> Option<Self> {
                $(
                    if name.eq_ignore_ascii_case($first) $(|| name.eq_ignore_ascii_case($more))* {
                        return Some(Self::$variant);
                    }
                )*
                None
            }
        }
    };
}

keywords! {
    /// The values of `display`.
    pub enum DisplaySpec {
        Flex = "flex",
        Block = "block",
        Grid = "grid",
        None = "none",
    }
}

keywords! {
    /// The values of `flex-direction`.
    pub enum FlexDirectionSpec {
        Row = "row",
        Column = "column",
        RowReverse = "row-reverse" | "row_reverse",
        ColumnReverse = "column-reverse" | "column_reverse",
    }
}

keywords! {
    /// The values of `align-items` and `justify-content`.
    pub enum AlignSpec {
        Start = "start" | "flex-start",
        End = "end" | "flex-end",
        Center = "center",
        Stretch = "stretch",
    }
}

keywords! {
    /// The values of `overflow`.
    pub enum OverflowSpec {
        Visible = "visible",
        /// Clips without making a scroll container.
        Clip = "clip",
        Hidden = "hidden",
        /// Scrolls, and a scrollbar of this axis can reserve width.
        Scroll = "scroll",
        /// Scrolls, and a scrollbar of this axis draws over the content.
        Auto = "auto",
    }
}

/// The value a style key takes. A value the kind does not accept is dropped
/// with a warning.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValueKind {
    /// A px value `>= 0`. A text beyond the largest length applies at it.
    NonNegativePx,
    /// A px value `>= 0`, or `"auto"`. A text beyond the largest length applies at it.
    NonNegativePxOrAuto,
    /// A px value, or `"auto"`. A text beyond the largest length applies at it.
    PxOrAuto,
    /// A px value `>= 0`, limited to the largest font size.
    FontSizePx,
    /// A decimal value `>= 0`. A text beyond the largest flex factor applies at it.
    NonNegativeNumber,
    /// A decimal value or a percentage, clamped to `0..=1` when used.
    Opacity,
    /// A number from `0` to `0xffffff`, or a string: `#rgb`, `#rgba`, `#rrggbb`,
    /// `#rrggbbaa`, `transparent`, `rgb()` or `rgba()`.
    Color,
    /// One of the names.
    Keyword(&'static [&'static str]),
    /// One or two of the names separated by whitespace: x, then y.
    KeywordPair(&'static [&'static str]),
}

/// A style key the host reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyleKey {
    /// The host key, in `snake_case`.
    pub key: &'static str,
    /// The CSS names of the same prop, in `snake_case`.
    pub aliases: &'static [&'static str],
    /// The value the key takes.
    pub kind: ValueKind,
}

/// Every style key the host reads, in `snake_case`, with its CSS names and
/// the value it takes.
pub const STYLE_KEYS: &[StyleKey] = &[
    StyleKey {
        key: "display",
        aliases: &[],
        kind: ValueKind::Keyword(DisplaySpec::NAMES),
    },
    StyleKey {
        key: "flex_direction",
        aliases: &[],
        kind: ValueKind::Keyword(FlexDirectionSpec::NAMES),
    },
    StyleKey {
        key: "justify_content",
        aliases: &[],
        kind: ValueKind::Keyword(AlignSpec::NAMES),
    },
    StyleKey {
        key: "align_items",
        aliases: &[],
        kind: ValueKind::Keyword(AlignSpec::NAMES),
    },
    StyleKey {
        key: "gap",
        aliases: &[],
        kind: ValueKind::NonNegativePx,
    },
    StyleKey {
        key: "width",
        aliases: &[],
        kind: ValueKind::NonNegativePxOrAuto,
    },
    StyleKey {
        key: "height",
        aliases: &[],
        kind: ValueKind::NonNegativePxOrAuto,
    },
    StyleKey {
        key: "border_width",
        aliases: &[],
        kind: ValueKind::NonNegativePx,
    },
    StyleKey {
        key: "background",
        aliases: &["background_color"],
        kind: ValueKind::Color,
    },
    StyleKey {
        key: "border_color",
        aliases: &[],
        kind: ValueKind::Color,
    },
    StyleKey {
        key: "corner_radius",
        aliases: &["border_radius"],
        kind: ValueKind::NonNegativePx,
    },
    StyleKey {
        key: "text_color",
        aliases: &["color"],
        kind: ValueKind::Color,
    },
    StyleKey {
        key: "text_size",
        aliases: &["font_size"],
        kind: ValueKind::FontSizePx,
    },
    StyleKey {
        key: "overflow",
        aliases: &[],
        kind: ValueKind::KeywordPair(OverflowSpec::NAMES),
    },
    StyleKey {
        key: "overflow_x",
        aliases: &[],
        kind: ValueKind::Keyword(OverflowSpec::NAMES),
    },
    StyleKey {
        key: "overflow_y",
        aliases: &[],
        kind: ValueKind::Keyword(OverflowSpec::NAMES),
    },
    StyleKey {
        key: "flex_grow",
        aliases: &[],
        kind: ValueKind::NonNegativeNumber,
    },
    StyleKey {
        key: "flex_shrink",
        aliases: &[],
        kind: ValueKind::NonNegativeNumber,
    },
    StyleKey {
        key: "opacity",
        aliases: &[],
        kind: ValueKind::Opacity,
    },
    StyleKey {
        key: "min_width",
        aliases: &[],
        kind: ValueKind::NonNegativePxOrAuto,
    },
    StyleKey {
        key: "min_height",
        aliases: &[],
        kind: ValueKind::NonNegativePxOrAuto,
    },
    StyleKey {
        key: "max_width",
        aliases: &[],
        kind: ValueKind::NonNegativePxOrAuto,
    },
    StyleKey {
        key: "max_height",
        aliases: &[],
        kind: ValueKind::NonNegativePxOrAuto,
    },
    StyleKey {
        key: "padding",
        aliases: &[],
        kind: ValueKind::NonNegativePx,
    },
    StyleKey {
        key: "padding_x",
        aliases: &[],
        kind: ValueKind::NonNegativePx,
    },
    StyleKey {
        key: "padding_y",
        aliases: &[],
        kind: ValueKind::NonNegativePx,
    },
    StyleKey {
        key: "padding_top",
        aliases: &[],
        kind: ValueKind::NonNegativePx,
    },
    StyleKey {
        key: "padding_right",
        aliases: &[],
        kind: ValueKind::NonNegativePx,
    },
    StyleKey {
        key: "padding_bottom",
        aliases: &[],
        kind: ValueKind::NonNegativePx,
    },
    StyleKey {
        key: "padding_left",
        aliases: &[],
        kind: ValueKind::NonNegativePx,
    },
    StyleKey {
        key: "margin",
        aliases: &[],
        kind: ValueKind::PxOrAuto,
    },
    StyleKey {
        key: "margin_x",
        aliases: &[],
        kind: ValueKind::PxOrAuto,
    },
    StyleKey {
        key: "margin_y",
        aliases: &[],
        kind: ValueKind::PxOrAuto,
    },
    StyleKey {
        key: "margin_top",
        aliases: &[],
        kind: ValueKind::PxOrAuto,
    },
    StyleKey {
        key: "margin_right",
        aliases: &[],
        kind: ValueKind::PxOrAuto,
    },
    StyleKey {
        key: "margin_bottom",
        aliases: &[],
        kind: ValueKind::PxOrAuto,
    },
    StyleKey {
        key: "margin_left",
        aliases: &[],
        kind: ValueKind::PxOrAuto,
    },
];

/// The host key of a style key. `key` may be `camelCase`, `kebab-case` or
/// `snake_case`, and a CSS name listed in [`STYLE_KEYS`] names its host key.
/// A key that is already a host key in `snake_case` comes back borrowed.
pub(crate) fn style_key(key: &str) -> Cow<'_, str> {
    let snake = if key.bytes().any(|b| b.is_ascii_uppercase() || b == b'-') {
        let mut out = String::with_capacity(key.len() + 2);
        for c in key.chars() {
            match c {
                '-' => out.push('_'),
                c if c.is_ascii_uppercase() => {
                    out.push('_');
                    out.push(c.to_ascii_lowercase());
                }
                c => out.push(c),
            }
        }
        Cow::Owned(out)
    } else {
        Cow::Borrowed(key)
    };
    match STYLE_KEYS
        .iter()
        .find(|entry| entry.aliases.contains(&&*snake))
    {
        Some(entry) => Cow::Borrowed(entry.key),
        None => snake,
    }
}

/// The CSS name of the host key `key`: its first CSS alias, or the key itself, in `kebab-case`.
pub(crate) fn css_name(key: &str) -> String {
    let name = STYLE_KEYS
        .iter()
        .find(|entry| entry.key == key)
        .and_then(|entry| entry.aliases.first().copied())
        .unwrap_or(key);
    name.replace('_', "-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::host_key("flex_direction")]
    #[case::css_name("color")]
    #[case::css_name_snake("font_size")]
    #[case::css_name_camel("backgroundColor")]
    fn style_key_borrows_a_key_that_needs_no_new_text(#[case] key: &str) {
        assert!(matches!(style_key(key), Cow::Borrowed(_)), "{key}");
    }

    #[rstest]
    #[case::camel("flexDirection", "flex_direction")]
    #[case::kebab("flex-direction", "flex_direction")]
    #[case::leading_capital("FlexDirection", "_flex_direction")]
    #[case::double_dash("--x", "__x")]
    #[case::dash_before_capital("flex-Direction", "flex__direction")]
    #[case::empty("", "")]
    #[case::multibyte_before_capital("é-Z", "é__z")]
    #[case::multibyte_only("幅", "幅")]
    #[case::css_color("color", "text_color")]
    #[case::css_background_color("backgroundColor", "background")]
    #[case::css_kebab("border-radius", "corner_radius")]
    #[case::css_snake("font_size", "text_size")]
    #[case::vendor_prefix("-webkit-x", "_webkit_x")]
    fn a_style_key_normalizes_to_snake_case_in_every_edge_case(
        #[case] key: &str,
        #[case] want: &str,
    ) {
        assert_eq!(style_key(key), want, "{key}");
    }

    #[test]
    fn a_css_name_reads_back_as_its_key() {
        for entry in STYLE_KEYS {
            assert_eq!(style_key(&css_name(entry.key)), entry.key, "{}", entry.key);
        }
    }

    #[rstest]
    #[case::alias_for_color("text_color", "color")]
    #[case::alias_for_radius("corner_radius", "border-radius")]
    #[case::first_alias("background", "background-color")]
    #[case::kebab_case("flex_direction", "flex-direction")]
    fn css_name_writes_the_css_spelling(#[case] key: &str, #[case] expected: &str) {
        assert_eq!(css_name(key), expected);
    }

    #[test]
    fn every_listed_alias_maps_to_its_key() {
        for entry in STYLE_KEYS {
            for alias in entry.aliases {
                assert!(
                    STYLE_KEYS.iter().all(|k| k.key != *alias),
                    "{alias} is also a key"
                );
                assert_eq!(style_key(alias), entry.key, "{alias}");
            }
        }
    }
}
