// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[test]
fn the_report_of_a_missing_entry_names_it() {
    let dir = ScratchDir::new("print-missing");
    let entry = dir.0.join("bundle.js").to_string_lossy().into_owned();

    assert_eq!(
        config_report(&entry),
        Err(format!("failed to read {entry}: not a file"))
    );
}

#[test]
fn the_report_reads_the_config_beside_the_entry() {
    let dir = ScratchDir::new("print-present");
    let entry = dir.0.join("bundle.js");
    fs::write(&entry, "").unwrap();
    fs::write(dir.0.join("inca.json"), r#"{"name":"Demo"}"#).unwrap();

    let report: serde_json::Value =
        serde_json::from_str(&config_report(&entry.to_string_lossy()).unwrap()).unwrap();
    assert_eq!(report["name"], "Demo");
}

/// A directory under the OS temp root, unique per test invocation, torn
/// down on drop. This crate has no `tempfile` dependency to reach for.
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new(name: &str) -> Self {
        let path = env::temp_dir().join(format!(
            "inca-host-test-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn bundle_beside_finds_a_flat_sibling_bundle() {
    let dir = ScratchDir::new("flat");
    fs::write(dir.0.join("bundle.js"), "").unwrap();

    assert_eq!(bundle_beside(&dir.0), Some(dir.0.join("bundle.js")));
}

#[test]
fn bundle_beside_finds_a_macos_app_bundles_resources() {
    let dir = ScratchDir::new("app-bundle");
    let macos_dir = dir.0.join("Contents/MacOS");
    let resources_dir = dir.0.join("Contents/Resources");
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

    assert_eq!(bundle_beside(&dir.0), None);
}

#[test]
fn a_chunk_importing_the_packaged_entry_back_does_not_evaluate_it_twice() {
    let dir = ScratchDir::new("entry-import-back");
    let macos_dir = dir.0.join("Contents/MacOS");
    let assets_dir = dir.0.join("Contents/Resources/assets");
    fs::create_dir_all(&macos_dir).unwrap();
    fs::create_dir_all(&assets_dir).unwrap();
    let entry = dir.0.join("Contents/Resources/bundle.js");
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
