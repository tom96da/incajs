// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The settings an app declares, as its build writes them beside the entry.

use std::fs;
use std::io;
use std::path::Path;

use gpui::{Pixels, Size, px, size};
use serde::{Deserialize, Serialize};

/// The file an app's build writes its config to, beside the entry.
const CONFIG_FILE_NAME: &str = "inca.json";

/// What an app declares about itself. Each field is absent unless the app's
/// config sets it.
#[derive(Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppConfig {
    /// Shown wherever the operating system presents the app to a user.
    pub name: Option<String>,
    /// A reverse-DNS id: Wayland's `app_id`, X11's `WM_CLASS`, and on
    /// Windows the `AppUserModelID`.
    pub identifier: Option<String>,
    /// The window the app opens in.
    pub window: Option<WindowConfig>,
}

/// What an app declares about its window.
#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WindowConfig {
    /// Initial width, in pixels.
    pub width: Option<f32>,
    /// Initial height, in pixels.
    pub height: Option<f32>,
    /// The window's title.
    pub title: Option<String>,
    /// Whether a user can resize the window.
    pub resizable: Option<bool>,
    /// Smallest width the window can be, in pixels. It opens at least this wide.
    pub min_width: Option<f32>,
    /// Smallest height the window can be, in pixels. It opens at least this tall.
    pub min_height: Option<f32>,
}

/// Reads the config beside `entry_path`, the app's entry file.
///
/// An app that declares nothing has no config file, and reads as
/// [`AppConfig::default`]. One that cannot be read or parsed is named on
/// stderr, and reads as [`AppConfig::default`] too.
#[must_use]
pub fn read(entry_path: &Path) -> AppConfig {
    let Some(path) = entry_path.parent().map(|dir| dir.join(CONFIG_FILE_NAME)) else {
        return AppConfig::default();
    };

    let json = match fs::read_to_string(&path) {
        Ok(json) => json,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return AppConfig::default(),
        Err(err) => {
            eprintln!("ignoring {}: {err}", path.display());
            return AppConfig::default();
        }
    };

    serde_json::from_str(&json).unwrap_or_else(|err| {
        eprintln!("ignoring {}: {err}", path.display());
        AppConfig::default()
    })
}

/// What an app is called when nothing named it.
pub(crate) const DEFAULT_APP_NAME: &str = "Inca";

/// Window size to fall back to when neither the app's config nor its root
/// element gives one.
pub(crate) const DEFAULT_WINDOW_SIZE: (f32, f32) = (800.0, 600.0);

/// A window dimension that is finite and above zero. Anything else reads
/// as absent.
pub(crate) fn usable(dimension: Option<f32>) -> Option<f32> {
    dimension.filter(|value| value.is_finite() && *value > 0.0)
}

/// The smallest size the window can be resized to, where the app's config
/// gives one. A dimension it leaves out is unconstrained.
pub(crate) fn window_min_size(window: Option<&WindowConfig>) -> Option<Size<Pixels>> {
    let min_width = usable(window.and_then(|w| w.min_width));
    let min_height = usable(window.and_then(|w| w.min_height));
    if min_width.is_none() && min_height.is_none() {
        return None;
    }
    Some(size(
        px(min_width.unwrap_or(0.0)),
        px(min_height.unwrap_or(0.0)),
    ))
}

/// The window's title: what the app's config asks for, then the app's own
/// name. A title of nothing but spaces counts as none.
pub(crate) fn window_title(window: Option<&WindowConfig>, name: Option<&str>) -> Option<String> {
    window
        .and_then(|window| window.title.clone())
        .filter(|title| !title.trim().is_empty())
        .or_else(|| name.map(ToOwned::to_owned))
}

/// Whether the app's config lets a user resize its window by hand.
/// Unconfigured reads as fixed-size.
pub(crate) fn is_resizable(window: Option<&WindowConfig>) -> bool {
    window.and_then(|w| w.resizable).unwrap_or(false)
}

/// The size to open the window at: what the app's config asks for, then
/// what its root element declares, then [`DEFAULT_WINDOW_SIZE`]. A window
/// never opens below the minimum it declared.
pub(crate) fn window_size(
    window: Option<&WindowConfig>,
    content: (Option<f32>, Option<f32>),
) -> (f32, f32) {
    let configured = |pick: fn(&WindowConfig) -> Option<f32>| usable(window.and_then(pick));
    let at_least = |pick: fn(&WindowConfig) -> Option<f32>| configured(pick).unwrap_or(0.0);
    (
        configured(|w| w.width)
            .or(usable(content.0))
            .unwrap_or(DEFAULT_WINDOW_SIZE.0)
            .max(at_least(|w| w.min_width)),
        configured(|w| w.height)
            .or(usable(content.1))
            .unwrap_or(DEFAULT_WINDOW_SIZE.1)
            .max(at_least(|w| w.min_height)),
    )
}

/// The settings the host applies for `config`, as pretty-printed JSON. A
/// `null` marks a value the config leaves to the host's own choice: the
/// app's root element sizes the window, and an unset limit stays open.
pub(crate) fn effective_config(config: &AppConfig) -> String {
    let window = config.window.as_ref();
    let effective = serde_json::json!({
        "name": config.name.as_deref().unwrap_or(DEFAULT_APP_NAME),
        "identifier": config.identifier,
        "window": {
            "title": window_title(window, config.name.as_deref()),
            "width": usable(window.and_then(|w| w.width)),
            "height": usable(window.and_then(|w| w.height)),
            "resizable": is_resizable(window),
            "minWidth": usable(window.and_then(|w| w.min_width)),
            "minHeight": usable(window.and_then(|w| w.min_height)),
        },
    });
    format!("{effective:#}")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::env;
    use std::path::PathBuf;

    /// A directory holding an entry file, and whatever config sits beside it.
    struct AppDir(PathBuf);

    impl AppDir {
        fn new(name: &str, config: Option<&str>) -> Self {
            let dir = env::temp_dir().join(format!("inca-config-{name}-{}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("bundle.js"), "").unwrap();
            match config {
                Some(json) => fs::write(dir.join(CONFIG_FILE_NAME), json).unwrap(),
                None => drop(fs::remove_file(dir.join(CONFIG_FILE_NAME))),
            }
            Self(dir)
        }

        fn entry(&self) -> PathBuf {
            self.0.join("bundle.js")
        }
    }

    impl Drop for AppDir {
        fn drop(&mut self) {
            drop(fs::remove_dir_all(&self.0));
        }
    }

    #[test]
    fn an_entry_with_no_config_beside_it_reads_as_the_defaults() {
        let app = AppDir::new("absent", None);

        assert_eq!(read(&app.entry()), AppConfig::default());
    }

    #[test]
    fn every_field_is_read() {
        let app = AppDir::new(
            "full",
            Some(
                r#"
                {"name":"Demo","identifier":"org.inca.demo",
                "window":{"width":1024,"height":768,"title":"Demo Window",
                          "resizable":true,"minWidth":320,"minHeight":240}}
                "#,
            ),
        );

        let config = read(&app.entry());

        assert_eq!(config.name.as_deref(), Some("Demo"));
        assert_eq!(config.identifier.as_deref(), Some("org.inca.demo"));
        let window = config.window.unwrap();
        assert_eq!(window.width, Some(1024.0));
        assert_eq!(window.height, Some(768.0));
        assert_eq!(window.title.as_deref(), Some("Demo Window"));
        assert_eq!(window.resizable, Some(true));
        assert_eq!(window.min_width, Some(320.0));
        assert_eq!(window.min_height, Some(240.0));
    }

    #[test]
    fn a_field_the_app_left_out_reads_as_none() {
        let app = AppDir::new("partial", Some(r#"{"window":{"width":320}}"#));

        let config = read(&app.entry());

        assert_eq!(config.name, None);
        assert_eq!(config.identifier, None);
        let window = config.window.unwrap();
        assert_eq!(window.width, Some(320.0));
        assert_eq!(window.height, None);
        assert_eq!(window.title, None);
        assert_eq!(window.resizable, None);
        assert_eq!(window.min_width, None);
        assert_eq!(window.min_height, None);
    }

    #[test]
    fn a_config_that_is_not_a_json_object_reads_as_the_defaults() {
        let app = AppDir::new("broken", Some("not json"));

        assert_eq!(read(&app.entry()), AppConfig::default());
    }

    #[test]
    fn a_wrongly_typed_field_reads_as_the_defaults() {
        let app = AppDir::new("mistyped", Some(r#"{"window":{"width":"big"}}"#));

        assert_eq!(read(&app.entry()), AppConfig::default());
    }

    #[test]
    fn a_dimension_no_window_can_open_at_falls_through() {
        let unusable = WindowConfig {
            width: Some(-100.0),
            height: Some(0.0),
            min_width: Some(f32::INFINITY),
            min_height: Some(f32::NAN),
            ..WindowConfig::default()
        };

        assert_eq!(
            window_size(Some(&unusable), (Some(300.0), None)),
            (300.0, DEFAULT_WINDOW_SIZE.1)
        );
        assert_eq!(window_min_size(Some(&unusable)), None);
    }

    #[test]
    fn an_unusable_content_dimension_falls_through_too() {
        assert_eq!(
            window_size(None, (Some(f32::NAN), None)),
            DEFAULT_WINDOW_SIZE
        );
    }

    #[test]
    fn window_min_size_carries_both_dimensions_when_both_are_usable() {
        let both = WindowConfig {
            min_width: Some(320.0),
            min_height: Some(240.0),
            ..WindowConfig::default()
        };

        assert_eq!(
            window_min_size(Some(&both)),
            Some(size(px(320.0), px(240.0)))
        );
    }

    #[test]
    fn window_min_size_keeps_the_dimension_that_is_usable() {
        let half_usable = WindowConfig {
            min_width: Some(320.0),
            min_height: Some(f32::NAN),
            ..WindowConfig::default()
        };

        assert_eq!(
            window_min_size(Some(&half_usable)),
            Some(size(px(320.0), px(0.0)))
        );
    }

    #[test]
    fn a_window_opens_no_smaller_than_the_minimum_it_declared() {
        let below_its_minimum = WindowConfig {
            width: Some(200.0),
            min_width: Some(320.0),
            ..WindowConfig::default()
        };

        assert_eq!(
            window_size(Some(&below_its_minimum), (None, None)),
            (320.0, DEFAULT_WINDOW_SIZE.1)
        );
    }

    #[test]
    fn a_minimum_above_the_default_raises_a_window_the_app_did_not_size() {
        let tall = WindowConfig {
            min_width: Some(2000.0),
            ..WindowConfig::default()
        };

        assert_eq!(
            window_size(Some(&tall), (None, None)),
            (2000.0, DEFAULT_WINDOW_SIZE.1)
        );
    }

    #[test]
    fn a_title_of_nothing_but_spaces_leaves_the_window_named_after_the_app() {
        for blank in ["", "   "] {
            let window = WindowConfig {
                title: Some(blank.to_owned()),
                ..WindowConfig::default()
            };

            assert_eq!(
                window_title(Some(&window), Some("Demo")),
                Some("Demo".to_owned()),
                "{blank:?}"
            );
        }
    }

    #[test]
    fn a_title_the_app_set_wins_over_its_name() {
        let window = WindowConfig {
            title: Some("Window".to_owned()),
            ..WindowConfig::default()
        };

        assert_eq!(
            window_title(Some(&window), Some("Demo")),
            Some("Window".to_owned())
        );
        assert_eq!(window_title(None, Some("Demo")), Some("Demo".to_owned()));
        assert_eq!(window_title(None, None), None);
    }

    #[test]
    fn window_min_size_is_absent_until_the_config_asks_for_one() {
        assert_eq!(window_min_size(None), None);
        assert_eq!(window_min_size(Some(&WindowConfig::default())), None);
    }

    #[test]
    fn window_min_size_leaves_a_dimension_the_config_omits_unconstrained() {
        let width_only = WindowConfig {
            min_width: Some(320.0),
            ..WindowConfig::default()
        };

        assert_eq!(
            window_min_size(Some(&width_only)),
            Some(size(px(320.0), px(0.0)))
        );
    }

    #[test]
    fn window_size_prefers_the_config_over_the_mounted_content() {
        let configured = WindowConfig {
            width: Some(1024.0),
            height: Some(768.0),
            ..WindowConfig::default()
        };

        assert_eq!(
            window_size(Some(&configured), (Some(300.0), Some(150.0))),
            (1024.0, 768.0)
        );
    }

    #[test]
    fn window_size_falls_through_each_source_per_dimension() {
        let width_only = WindowConfig {
            width: Some(1024.0),
            ..WindowConfig::default()
        };

        assert_eq!(
            window_size(Some(&width_only), (Some(300.0), Some(150.0))),
            (1024.0, 150.0)
        );
        assert_eq!(
            window_size(Some(&width_only), (None, None)),
            (1024.0, DEFAULT_WINDOW_SIZE.1)
        );
        assert_eq!(window_size(None, (None, None)), DEFAULT_WINDOW_SIZE);
    }

    fn effective(json: &str) -> serde_json::Value {
        let config: AppConfig = serde_json::from_str(json).unwrap();
        serde_json::from_str(&effective_config(&config)).unwrap()
    }

    #[test]
    fn an_app_that_declares_nothing_prints_the_defaults() {
        assert_eq!(
            effective("{}"),
            serde_json::json!({
                "name": "Inca",
                "identifier": null,
                "window": {
                    "title": null,
                    "width": null,
                    "height": null,
                    "resizable": false,
                    "minWidth": null,
                    "minHeight": null,
                },
            })
        );
    }

    #[test]
    fn every_declared_setting_is_printed() {
        assert_eq!(
            effective(
                r#"
                {"name":"Demo","identifier":"org.inca.demo",
                "window":{"width":1024,"height":768,"title":"Window",
                          "resizable":true,"minWidth":320,"minHeight":240}}
                "#
            ),
            serde_json::json!({
                "name": "Demo",
                "identifier": "org.inca.demo",
                "window": {
                    "title": "Window",
                    "width": 1024.0,
                    "height": 768.0,
                    "resizable": true,
                    "minWidth": 320.0,
                    "minHeight": 240.0,
                },
            })
        );
    }

    #[test]
    fn the_window_title_falls_back_to_the_name() {
        let printed = effective(r#"{"name":"Demo","window":{"title":"  "}}"#);

        assert_eq!(printed["window"]["title"], "Demo");
        assert_eq!(effective(r#"{"name":"Demo"}"#)["window"]["title"], "Demo");
    }

    #[test]
    fn a_size_no_window_can_open_at_prints_as_null() {
        let printed =
            effective(r#"{"window":{"width":-5,"height":0,"minWidth":-1,"minHeight":0}}"#);

        for key in ["width", "height", "minWidth", "minHeight"] {
            assert!(printed["window"][key].is_null(), "{key}");
        }
    }
}
