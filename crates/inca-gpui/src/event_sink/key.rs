// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The key names of keyboard events.

use super::Keystroke;

/// Whether gpui's `key` names the Menu key: `"menu"`, or U+0010 from a Mac keyboard.
#[must_use]
pub fn is_menu_key(key: &str) -> bool {
    matches!(key, "menu" | "\u{10}")
}

/// GPUI's `Keystroke::key` to the key name, covering GPUI's
/// special key names, grown as real usage needs more of them. Falls
/// through to `key_char` (the character actually typed, when the platform
/// reports one) and then to `key` itself for anything not in the table.
pub(super) fn key_name(keystroke: &Keystroke) -> String {
    if is_menu_key(&keystroke.key) {
        return "ContextMenu".to_string();
    }
    let named = match keystroke.key.as_str() {
        "space" => Some(" "),
        "tab" => Some("Tab"),
        "enter" => Some("Enter"),
        "backspace" => Some("Backspace"),
        "delete" => Some("Delete"),
        "left" => Some("ArrowLeft"),
        "right" => Some("ArrowRight"),
        "up" => Some("ArrowUp"),
        "down" => Some("ArrowDown"),
        "pageup" => Some("PageUp"),
        "pagedown" => Some("PageDown"),
        "insert" => Some("Insert"),
        "home" => Some("Home"),
        "end" => Some("End"),
        "back" => Some("BrowserBack"),
        "forward" => Some("BrowserForward"),
        "escape" => Some("Escape"),
        _ => None,
    };
    if let Some(named) = named {
        return named.to_string();
    }
    if let Some(function) = keystroke
        .key
        .strip_prefix('f')
        .filter(|n| n.parse::<u8>().is_ok_and(|n| (1..=35).contains(&n)))
    {
        return format!("F{function}");
    }
    if let Some(typed) = keystroke.key_char.as_ref().filter(|c| !c.is_empty()) {
        return typed.clone();
    }
    // A shifted letter with no `key_char` (a shortcut) keeps its case.
    let mut letter = keystroke.key.chars();
    match (letter.next(), letter.next()) {
        (Some(c), None) if c.is_ascii_lowercase() && keystroke.modifiers.shift => {
            c.to_ascii_uppercase().to_string()
        }
        (None, _) => "Unidentified".to_string(),
        _ => keystroke.key.clone(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;
    use rstest::rstest;

    #[test]
    fn key_name_prefers_a_named_key_over_its_key_char() {
        let enter = Keystroke {
            key: "enter".to_string(),
            key_char: Some("\n".to_string()),
            modifiers: gpui::Modifiers::default(),
        };
        assert_eq!(key_name(&enter), "Enter");
    }

    #[test]
    fn key_name_names_every_function_key() {
        for n in 1..=35u8 {
            let keystroke = Keystroke {
                key: format!("f{n}"),
                key_char: None,
                modifiers: gpui::Modifiers::default(),
            };
            assert_eq!(key_name(&keystroke), format!("F{n}"));
        }
    }

    #[rstest]
    #[case::linux_and_windows("menu")]
    #[case::macos("\u{10}")]
    fn key_name_names_the_menu_key_context_menu(#[case] key: &str) {
        let menu = Keystroke {
            key: key.to_string(),
            key_char: None,
            modifiers: gpui::Modifiers::default(),
        };
        assert_eq!(key_name(&menu), "ContextMenu");
    }

    #[test]
    fn key_name_falls_back_to_key_char_for_an_unnamed_key() {
        let shifted_s = Keystroke {
            key: "s".to_string(),
            key_char: Some("S".to_string()),
            modifiers: gpui::Modifiers::default(),
        };
        assert_eq!(key_name(&shifted_s), "S");
    }

    #[test]
    fn key_name_falls_back_to_key_when_key_char_is_none() {
        let cmd_s = Keystroke {
            key: "s".to_string(),
            key_char: None,
            modifiers: gpui::Modifiers::default(),
        };
        assert_eq!(key_name(&cmd_s), "s");
    }

    #[test]
    fn key_name_covers_the_unidentified_and_shifted_fallbacks() {
        let shift = gpui::Modifiers {
            shift: true,
            ..gpui::Modifiers::default()
        };
        let none = gpui::Modifiers::default();
        // (key, key_char, modifiers, expected)
        let cases = [
            ("", None, none, "Unidentified"),
            ("", Some(""), none, "Unidentified"),
            ("s", Some(""), none, "s"),
            ("s", None, shift, "S"),
            ("1", None, shift, "1"),
            ("shift", None, shift, "shift"),
            ("s", Some("S"), shift, "S"),
        ];
        let mismatches: Vec<_> = cases
            .iter()
            .filter_map(|&(key, key_char, modifiers, expected)| {
                let keystroke = Keystroke {
                    key: key.to_string(),
                    key_char: key_char.map(str::to_string),
                    modifiers,
                };
                let got = key_name(&keystroke);
                (got != expected).then(|| format!("{key:?}/{key_char:?}: {got} != {expected}"))
            })
            .collect();
        assert!(mismatches.is_empty(), "{mismatches:?}");
    }
}
