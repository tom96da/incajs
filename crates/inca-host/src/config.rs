// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The settings an app declares, as its build writes them beside the entry.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use gpui::{Pixels, Size, px, size};
use inca_jsenv::StackPaths;
use serde::{Deserialize, Serialize};
use serde_path_to_error::Segment;

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
    /// What only the development window applies.
    pub dev: Option<DevConfig>,
    /// The app's root, relative to the directory of this file.
    pub root: Option<String>,
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

/// What an app declares for the development window.
#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DevConfig {
    /// How a stack names a file.
    pub stack_paths: Option<StackPathStyle>,
}

/// How a stack names a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StackPathStyle {
    /// From the app's root when the file lies under it.
    Relative,
    /// In full.
    Absolute,
}

/// Why the config beside an entry cannot be used.
#[derive(Debug)]
pub enum ReadError {
    /// The file exists and cannot be read.
    Io(PathBuf, io::Error),
    /// The file is not JSON, or a value in it has a type the host does not
    /// take.
    Parse(PathBuf, serde_path_to_error::Error<serde_json::Error>),
}

impl ReadError {
    /// `<file>: <cause>`.
    fn summary(&self) -> String {
        match self {
            Self::Io(file, err) => format!("{}: {err}", file.display()),
            Self::Parse(file, err) => format!("{}: {}", file.display(), err.inner()),
        }
    }

    /// `<file>: <cause>`, with the path to the value before the cause of a
    /// parse error: `<file>: <path>: <cause>`. The path is a JSON array of
    /// the keys and indices from the top of the file, `null` for a step the
    /// parser could not name.
    pub(crate) fn report(&self) -> String {
        match self {
            Self::Io(..) => self.summary(),
            Self::Parse(file, err) => {
                format!(
                    "{}: {}: {}",
                    file.display(),
                    json_path(err.path()),
                    err.inner()
                )
            }
        }
    }
}

/// `path` as a compact JSON array: a key is a string, an index a number,
/// a step without a name `null`.
fn json_path(path: &serde_path_to_error::Path) -> String {
    let segments: Vec<serde_json::Value> = path
        .iter()
        .map(|segment| match segment {
            Segment::Seq { index } => (*index).into(),
            Segment::Map { key } | Segment::Enum { variant: key } => key.as_str().into(),
            Segment::Unknown => serde_json::Value::Null,
        })
        .collect();
    serde_json::Value::from(segments).to_string()
}

/// Reads the config beside `entry_path`, the app's entry file.
///
/// An app that declares nothing has no config file, and reads as
/// [`AppConfig::default`].
///
/// # Errors
///
/// Returns a [`ReadError`] when the file exists and cannot be read or
/// parsed.
pub fn try_read(entry_path: &Path) -> Result<AppConfig, ReadError> {
    let Some(path) = entry_path.parent().map(|dir| dir.join(CONFIG_FILE_NAME)) else {
        return Ok(AppConfig::default());
    };

    let json = match fs::read_to_string(&path) {
        Ok(json) => json,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(AppConfig::default()),
        Err(err) => return Err(ReadError::Io(path, err)),
    };

    parse(&json).map_err(|err| ReadError::Parse(path, err))
}

/// Parses `json` as [`serde_json::from_str`] does, keeping the path to the
/// value an error stops at. Text after the config fails with an empty path.
fn parse(json: &str) -> Result<AppConfig, serde_path_to_error::Error<serde_json::Error>> {
    let mut de = serde_json::Deserializer::from_str(json);
    let config = serde_path_to_error::deserialize(&mut de)?;
    de.end().map_err(|err| {
        serde_path_to_error::Error::new(serde_path_to_error::Track::new().path(), err)
    })?;
    Ok(config)
}

/// Reads the config beside `entry_path` as [`try_read`] does. One that
/// cannot be read or parsed is named on stderr, and reads as
/// [`AppConfig::default`].
#[must_use]
pub fn read(entry_path: &Path) -> AppConfig {
    try_read(entry_path).unwrap_or_else(|err| {
        eprintln!("ignoring {}", err.summary());
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

/// How the engine of the app at `entry_path` names the files of a stack. Without a root, or when
/// the config asks for it, a file is named in full.
pub(crate) fn stack_paths(config: &AppConfig, entry_path: &Path) -> StackPaths {
    let style = config.dev.as_ref().and_then(|dev| dev.stack_paths);
    let (Some(root), Some(StackPathStyle::Relative) | None) = (&config.root, style) else {
        return StackPaths::Absolute;
    };
    let dir = entry_path.parent().unwrap_or(Path::new("."));
    StackPaths::relative(&dir.join(root))
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
        "dev": {
            "stackPaths": match config.dev.as_ref().and_then(|dev| dev.stack_paths) {
                Some(StackPathStyle::Absolute) => "absolute",
                _ => "relative",
            },
        },
    });
    format!("{effective:#}")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::tests::ScratchDir;
    use rstest::rstest;
    use std::path::PathBuf;

    /// A directory holding an entry file, and whatever config sits beside it.
    fn app_dir(name: &str, config: Option<&str>) -> ScratchDir {
        let dir = ScratchDir::new(name);
        fs::write(dir.join("bundle.js"), "").unwrap();
        if let Some(json) = config {
            fs::write(dir.join(CONFIG_FILE_NAME), json).unwrap();
        }
        dir
    }

    fn entry(dir: &ScratchDir) -> PathBuf {
        dir.join("bundle.js")
    }

    #[test]
    fn an_entry_with_no_config_beside_it_reads_as_the_defaults() {
        let app = app_dir("absent", None);

        assert!(matches!(try_read(&entry(&app)), Ok(config) if config == AppConfig::default()));
    }

    #[test]
    fn every_field_is_read() {
        let app = app_dir(
            "full",
            Some(
                r#"
                {"name":"Demo","identifier":"org.inca.demo",
                "window":{"width":1024,"height":768,"title":"Demo Window",
                          "resizable":true,"minWidth":320,"minHeight":240},
                "dev":{"stackPaths":"absolute"},"root":".."}
                "#,
            ),
        );

        let config = read(&entry(&app));

        assert_eq!(config.name.as_deref(), Some("Demo"));
        assert_eq!(config.identifier.as_deref(), Some("org.inca.demo"));
        let window = config.window.unwrap();
        assert_eq!(window.width, Some(1024.0));
        assert_eq!(window.height, Some(768.0));
        assert_eq!(window.title.as_deref(), Some("Demo Window"));
        assert_eq!(window.resizable, Some(true));
        assert_eq!(window.min_width, Some(320.0));
        assert_eq!(window.min_height, Some(240.0));
        assert_eq!(
            config.dev.unwrap().stack_paths,
            Some(StackPathStyle::Absolute)
        );
        assert_eq!(config.root.as_deref(), Some(".."));
    }

    #[test]
    fn a_config_that_is_not_a_json_object_reads_as_the_defaults() {
        let app = app_dir("broken", Some("not json"));

        assert_eq!(read(&entry(&app)), AppConfig::default());
    }

    #[test]
    fn a_config_that_is_a_directory_is_an_error_naming_it() {
        let app = app_dir("unusable", None);
        fs::create_dir(app.join(CONFIG_FILE_NAME)).unwrap();

        let report = try_read(&entry(&app)).unwrap_err().report();

        let file = app.join(CONFIG_FILE_NAME);
        assert!(
            report.starts_with(&format!("{}: ", file.display())),
            "{report}"
        );
    }

    #[rstest]
    #[case::whole_file("not json", "[]")]
    #[case::key(r#"{"window":{"minWidth":"x"}}"#, r#"["window","minWidth"]"#)]
    #[case::index(r#"{"window":[1,"x"]}"#, r#"["window",1]"#)]
    #[case::unnamed(r#"{"window":{"width":1"#, r#"["window",null]"#)]
    #[case::trailing(r#"{"name":"Demo"} x"#, "[]")]
    fn a_parse_error_reports_the_path_to_the_value(#[case] json: &str, #[case] path: &str) {
        let app = app_dir("path", Some(json));

        let report = try_read(&entry(&app)).unwrap_err().report();

        let file = app.join(CONFIG_FILE_NAME);
        assert!(
            report.starts_with(&format!("{}: {path}: ", file.display())),
            "{report}"
        );
    }

    #[test]
    fn the_summary_of_a_parse_error_leaves_the_path_out() {
        let app = app_dir("summary", Some(r#"{"window":{"width":"wide"}}"#));

        let summary = try_read(&entry(&app)).unwrap_err().summary();

        let file = app.join(CONFIG_FILE_NAME);
        assert!(
            summary.starts_with(&format!("{}: ", file.display())),
            "{summary}"
        );
        assert!(!summary.contains(r#"["window""#), "{summary}");
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

    #[rstest]
    #[case::empty("")]
    #[case::spaces("   ")]
    fn a_title_of_nothing_but_spaces_leaves_the_window_named_after_the_app(#[case] blank: &str) {
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
                "dev": { "stackPaths": "relative" },
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
                          "resizable":true,"minWidth":320,"minHeight":240},
                "dev":{"stackPaths":"absolute"}}
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
                "dev": { "stackPaths": "absolute" },
            })
        );
    }

    #[rstest]
    #[case::relative_with_root(
        r#"{"dev":{"stackPaths":"relative"},"root":".."}"#,
        StackPaths::relative(&Path::new("/app/dist").join(".."))
    )]
    #[case::absolute_with_root(
        r#"{"dev":{"stackPaths":"absolute"},"root":".."}"#,
        StackPaths::Absolute
    )]
    #[case::unset_style_with_root(
        r#"{"dev":{},"root":".."}"#,
        StackPaths::relative(&Path::new("/app/dist").join(".."))
    )]
    #[case::no_dev_member_with_root(
        r#"{"root":".."}"#,
        StackPaths::relative(&Path::new("/app/dist").join(".."))
    )]
    #[case::no_root(r#"{"dev":{"stackPaths":"relative"}}"#, StackPaths::Absolute)]
    #[case::missing_root_dir(
        r#"{"root":"../gone"}"#,
        StackPaths::Relative { lexical: PathBuf::from("/app/gone"), real: None }
    )]
    fn stack_paths_follows_the_config(#[case] json: &str, #[case] expected: StackPaths) {
        let config: AppConfig = serde_json::from_str(json).unwrap();

        assert_eq!(
            stack_paths(&config, Path::new("/app/dist/bundle.js")),
            expected
        );
    }

    #[test]
    fn a_stack_path_style_other_than_the_two_fails_to_parse() {
        let app = app_dir("style", Some(r#"{"dev":{"stackPaths":"rel"}}"#));

        let report = try_read(&entry(&app)).unwrap_err().report();

        assert!(report.contains(r#"["dev","stackPaths"]"#), "{report}");
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
