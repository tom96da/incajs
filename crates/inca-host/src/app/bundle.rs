// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The discovery of the bundle a packaged app runs.

use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Where a packaged app's bundle lives relative to `exe_dir`, tried in
/// order: a flat layout (Linux, `bundle.js` beside the executable) then a
/// macOS `.app`'s (`Contents/MacOS/<exe>` next to `Contents/Resources/`).
fn bundle_beside(exe_dir: &Path) -> Option<PathBuf> {
    [
        exe_dir.join("bundle.js"),
        exe_dir.join("../Resources/bundle.js"),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file())
}

/// Finds the `bundle.js` that ships beside the running executable.
///
/// Looks next to the executable, then in `../Resources` (a macOS app
/// bundle). Returns `None` when neither holds a file.
fn bundle_beside_exe() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    bundle_beside(exe.parent()?)
}

/// Why no bundle could be found.
#[derive(Debug, PartialEq, Eq)]
pub enum BundleError {
    /// Nothing sits beside the executable and `INCA_BUNDLE_PATH` is unset or empty.
    NotFound,
    /// `INCA_BUNDLE_PATH` is set and unusable. Holds the line to print on stderr.
    Refused(String),
}

/// The bundle found beside the executable (`beside`), else the one `variable` (the value of
/// `INCA_BUNDLE_PATH`) names.
/// The variable is read only when nothing sits beside the executable.
fn bundle_from(beside: Option<PathBuf>, variable: Option<&OsStr>) -> Result<PathBuf, BundleError> {
    if let Some(path) = beside {
        return Ok(path);
    }
    let Some(value) = variable.filter(|value| !value.is_empty()) else {
        return Err(BundleError::NotFound);
    };
    let shown = value.to_string_lossy();
    let Some(text) = value.to_str() else {
        return Err(BundleError::Refused(format!(
            "INCA_BUNDLE_PATH names no file: {shown}"
        )));
    };
    let path = Path::new(text);
    if !path.is_absolute() {
        return Err(BundleError::Refused(format!(
            "INCA_BUNDLE_PATH must be an absolute path: {shown}"
        )));
    }
    if !path.is_file() {
        return Err(BundleError::Refused(format!(
            "INCA_BUNDLE_PATH names no file: {shown}"
        )));
    }
    Ok(path.to_path_buf())
}

/// Finds the `bundle.js` to run when argv names none: the one beside the executable, else the
/// one `INCA_BUNDLE_PATH` names.
///
/// # Errors
///
/// Returns [`BundleError::NotFound`] when neither exists and
/// [`BundleError::Refused`] when the variable is set and unusable.
pub fn launcher_bundle() -> Result<PathBuf, BundleError> {
    bundle_from(
        bundle_beside_exe(),
        env::var_os("INCA_BUNDLE_PATH").as_deref(),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::fs;

    use inca_jsenv::stderr_reporter;
    use rstest::rstest;

    use super::*;
    use crate::app::Session;
    use crate::tests::ScratchDir;

    #[test]
    fn bundle_beside_finds_a_flat_sibling_bundle() {
        let dir = ScratchDir::new("flat");
        fs::write(dir.join("bundle.js"), "").unwrap();

        assert_eq!(bundle_beside(&dir), Some(dir.join("bundle.js")));
    }

    #[test]
    fn bundle_beside_finds_a_macos_app_bundles_resources() {
        let dir = ScratchDir::new("app-bundle");
        let macos_dir = dir.join("Contents/MacOS");
        let resources_dir = dir.join("Contents/Resources");
        fs::create_dir_all(&macos_dir).unwrap();
        fs::create_dir_all(&resources_dir).unwrap();
        fs::write(resources_dir.join("bundle.js"), "").unwrap();

        assert_eq!(
            bundle_beside(&macos_dir),
            Some(macos_dir.join("../Resources/bundle.js"))
        );
    }

    #[test]
    fn bundle_beside_is_none_when_nothing_is_there() {
        let dir = ScratchDir::new("empty");

        assert_eq!(bundle_beside(&dir), None);
    }

    #[test]
    fn a_bundle_beside_wins_over_the_variable() {
        let dir = ScratchDir::new("var-beside");
        let other = dir.join("other.js");
        fs::write(&other, "").unwrap();

        assert_eq!(
            bundle_from(
                Some(PathBuf::from("/beside/bundle.js")),
                Some(other.as_os_str())
            ),
            Ok(PathBuf::from("/beside/bundle.js"))
        );
    }

    #[test]
    fn an_unusable_variable_is_not_read_when_a_bundle_is_beside() {
        assert_eq!(
            bundle_from(
                Some(PathBuf::from("/beside/bundle.js")),
                Some(OsStr::new("relative.js"))
            ),
            Ok(PathBuf::from("/beside/bundle.js"))
        );
    }

    #[test]
    fn a_usable_variable_names_the_bundle_when_nothing_is_beside() {
        let dir = ScratchDir::new("var-usable");
        let bundle = dir.join("bundle.js");
        fs::write(&bundle, "").unwrap();

        assert_eq!(bundle_from(None, Some(bundle.as_os_str())), Ok(bundle));
    }

    #[rstest]
    #[case::unset(None)]
    #[case::empty(Some(OsStr::new("")))]
    fn an_unset_or_empty_variable_reports_the_search_failure(#[case] variable: Option<&OsStr>) {
        assert_eq!(bundle_from(None, variable), Err(BundleError::NotFound));
    }

    #[test]
    fn a_relative_variable_is_refused() {
        assert_eq!(
            bundle_from(None, Some(OsStr::new("bundle.js"))),
            Err(BundleError::Refused(
                "INCA_BUNDLE_PATH must be an absolute path: bundle.js".to_string()
            ))
        );
    }

    fn missing(dir: &Path) -> PathBuf {
        dir.join("none.js")
    }

    fn directory(dir: &Path) -> PathBuf {
        dir.to_path_buf()
    }

    // macOS refuses a file name with invalid UTF-8, so the file only exists on Linux.
    #[cfg(target_os = "linux")]
    fn non_utf8(dir: &Path) -> PathBuf {
        use std::os::unix::ffi::OsStrExt;

        let bundle = dir.join(OsStr::from_bytes(b"bundle\xff.js"));
        fs::write(&bundle, "").unwrap();
        bundle
    }

    #[rstest]
    #[case::missing(missing)]
    #[case::directory(directory)]
    #[cfg_attr(target_os = "linux", case::non_utf8(non_utf8))]
    fn a_variable_that_names_no_file_is_refused(#[case] make: fn(&Path) -> PathBuf) {
        let dir = ScratchDir::new("var-no-file");
        let value = make(&dir);

        assert_eq!(
            bundle_from(None, Some(value.as_os_str())),
            Err(BundleError::Refused(format!(
                "INCA_BUNDLE_PATH names no file: {}",
                value.display()
            )))
        );
    }

    #[test]
    fn a_chunk_importing_the_packaged_entry_back_does_not_evaluate_it_twice() {
        let dir = ScratchDir::new("entry-import-back");
        let macos_dir = dir.join("Contents/MacOS");
        let assets_dir = dir.join("Contents/Resources/assets");
        fs::create_dir_all(&macos_dir).unwrap();
        fs::create_dir_all(&assets_dir).unwrap();
        let entry = dir.join("Contents/Resources/bundle.js");
        fs::write(
            &entry,
            r"
            globalThis.evaluations = (globalThis.evaluations ?? 0) + 1;

            import('./assets/chunk.js');
            ",
        )
        .unwrap();
        fs::write(assets_dir.join("chunk.js"), "import '../bundle.js';").unwrap();

        let path = bundle_beside(&macos_dir).unwrap();
        let session = Session::load(
            &path.to_string_lossy(),
            &fs::read_to_string(&path).unwrap(),
            stderr_reporter(),
        )
        .unwrap();
        assert!(session.engine.run_jobs().is_empty());

        let evaluations: i32 = session.engine.eval("globalThis.evaluations").unwrap();
        assert_eq!(evaluations, 1);
    }
}
