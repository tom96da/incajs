// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn the_report_of_a_missing_entry_names_it() {
    let dir = ScratchDir::new("print-missing");
    let entry = dir.join("bundle.js").to_string_lossy().into_owned();

    assert_eq!(
        config_report(&entry),
        Err(format!("failed to read {entry}: not a file"))
    );
}

#[test]
fn the_report_reads_the_config_beside_the_entry() {
    let dir = ScratchDir::new("print-present");
    let entry = dir.join("bundle.js");
    fs::write(&entry, "").unwrap();
    fs::write(dir.join("inca.json"), r#"{"name":"Demo"}"#).unwrap();

    let report: serde_json::Value =
        serde_json::from_str(&config_report(&entry.to_string_lossy()).unwrap()).unwrap();
    assert_eq!(report["name"], "Demo");
}

#[test]
fn the_report_of_a_config_it_cannot_parse_names_the_field() {
    let dir = ScratchDir::new("print-malformed");
    let entry = dir.join("bundle.js");
    fs::write(&entry, "").unwrap();
    fs::write(dir.join("inca.json"), r#"{"window":{"width":"wide"}}"#).unwrap();

    let message = config_report(&entry.to_string_lossy()).unwrap_err();

    let config = dir.join("inca.json");
    assert!(
        message.starts_with(&format!(
            r#"failed to read {}: ["window","width"]: "#,
            config.display()
        )),
        "{message}"
    );
}

#[test]
fn config_report_refuses_an_unusable_value_with_its_path() {
    let dir = ScratchDir::new("print-unusable");
    let entry = dir.join("bundle.js");
    fs::write(&entry, "").unwrap();
    fs::write(dir.join("inca.json"), r#"{"window":{"width":-100}}"#).unwrap();

    assert_eq!(
        config_report(&entry.to_string_lossy()),
        Err(format!(
            concat!(
                r#"failed to read {}: ["window","width"]: "#,
                "invalid value: -100, expected a finite number above zero"
            ),
            dir.join("inca.json").display()
        ))
    );
}

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
