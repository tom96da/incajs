// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The application menu, and the actions its items invoke.

use gpui::{App, KeyBinding, Menu, MenuItem, actions};

actions!(
    inca,
    [
        /// Quit the application.
        Quit,
    ]
);

/// What quits the app: `cmd-q` on macOS, `ctrl-q` elsewhere. macOS reads a
/// menu item's key equivalent from the keymap, so the shortcut shown beside
/// the Quit item comes from this binding.
const QUIT_KEYSTROKE: &str = "secondary-q";

/// One entry per top-level menu.
///
/// `app_name` titles the application menu. macOS titles it from the `.app`
/// bundle's `CFBundleName`.
fn menus(app_name: &str) -> Vec<Menu> {
    vec![Menu::new(app_name.to_owned()).items([MenuItem::action("Quit", Quit)])]
}

/// Installs the application menu, the actions its items invoke, and their
/// keyboard shortcuts. Call it before the first window opens.
pub fn install(cx: &mut App, app_name: &str) {
    cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
    cx.bind_keys([KeyBinding::new(QUIT_KEYSTROKE, Quit, None)]);
    cx.set_menus(menus(app_name));
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_app_names_its_own_menu() {
        let menus = menus("Demo");

        assert_eq!(menus.len(), 1);
        assert_eq!(menus[0].name.as_ref(), "Demo");
    }

    #[test]
    fn quit_is_the_one_item_every_app_gets() {
        let menus = menus("Demo");

        let [MenuItem::Action { name, action, .. }] = menus[0].items.as_slice() else {
            panic!("expected exactly one action item");
        };
        assert_eq!(name.as_ref(), "Quit");
        assert!(action.as_any().is::<Quit>());
    }

    #[test]
    fn the_quit_shortcut_is_the_platforms_own() {
        let binding = KeyBinding::new(QUIT_KEYSTROKE, Quit, None);
        let keystrokes = binding.keystrokes();

        assert_eq!(keystrokes.len(), 1);
        assert_eq!(keystrokes[0].key(), "q");
        assert_eq!(
            keystrokes[0].modifiers().platform,
            cfg!(target_os = "macos")
        );
        assert_eq!(
            keystrokes[0].modifiers().control,
            !cfg!(target_os = "macos")
        );
    }
}
