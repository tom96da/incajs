// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The settings an app declares, as its build writes them beside the entry.

use std::fs;
use std::io;
use std::path::Path;

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
#[derive(Debug, Default, PartialEq, Deserialize, Serialize)]
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
    /// Smallest width a user can resize the window to, in pixels.
    pub min_width: Option<f32>,
    /// Smallest height a user can resize the window to, in pixels.
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
                r#"{"name":"Demo","identifier":"org.inca.demo",
                    "window":{"width":1024,"height":768,"title":"Demo Window",
                              "resizable":true,"minWidth":320,"minHeight":240}}"#,
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
}
