// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Holds `inca.json`'s two sides to the same shape: the `RuntimeConfig`
//! `@incajs/cli` writes, and the `AppConfig` `inca-host` reads.
//!
//! `emit-config.mts` beside this file writes one with every member the
//! TypeScript side knows about, and `Required` there keeps it exhaustive.
//! Comparing the two key sets catches a member only one side has; looking
//! for a `null` catches one whose name the two sides spell differently.

#![allow(clippy::unwrap_used)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use inca_host::config::{self, AppConfig};
use serde_json::Value;

const EMIT_SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/emit-config.mts");

/// A directory holding one emitted `inca.json`, removed with the test.
/// `name` keeps tests that run at once out of each other's way.
struct EmittedConfig(PathBuf);

impl EmittedConfig {
    fn new(name: &str) -> Self {
        let dir = env::temp_dir().join(format!("inca-config-{name}-{}", std::process::id()));
        drop(fs::remove_dir_all(&dir));

        let status = Command::new("node")
            .arg(EMIT_SCRIPT)
            .arg(&dir)
            .status()
            .expect("node is needed to run emit-config.mts");
        assert!(status.success(), "{EMIT_SCRIPT} failed");

        // `config::read` looks beside the app's entry file.
        fs::write(dir.join("bundle.js"), "").unwrap();
        Self(dir)
    }

    fn entry(&self) -> PathBuf {
        self.0.join("bundle.js")
    }

    fn json(&self) -> Value {
        serde_json::from_str(&fs::read_to_string(self.0.join("inca.json")).unwrap()).unwrap()
    }
}

impl Drop for EmittedConfig {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

/// Walks `value`, calling `visit` with each member's path and its value.
fn walk(value: &Value, prefix: &str, visit: &mut impl FnMut(String, &Value)) {
    let Some(object) = value.as_object() else {
        return;
    };
    for (key, member) in object {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        walk(member, &path, visit);
        visit(path, member);
    }
}

/// Every member's path in `value`, sorted.
fn keys_of(value: &Value) -> Vec<String> {
    let mut keys = Vec::new();
    walk(value, "", &mut |path, _| keys.push(path));
    keys.sort();
    keys
}

/// The paths in `value` that hold `null`, sorted.
fn nulls_in(value: &Value) -> Vec<String> {
    let mut nulls = Vec::new();
    walk(value, "", &mut |path, member| {
        if member.is_null() {
            nulls.push(path);
        }
    });
    nulls.sort();
    nulls
}

#[test]
fn both_sides_know_the_same_members() {
    let emitted = EmittedConfig::new("members");

    let host = serde_json::to_value(config::read(&emitted.entry())).unwrap();

    assert_eq!(keys_of(&host), keys_of(&emitted.json()));
}

#[test]
fn the_host_binds_every_member_the_emitted_config_carries() {
    let emitted = EmittedConfig::new("binding");

    let host = serde_json::to_value(config::read(&emitted.entry())).unwrap();

    assert_eq!(nulls_in(&host), Vec::<String>::new());
}

#[test]
fn a_config_the_build_never_wrote_leaves_the_host_at_its_defaults() {
    let dir = env::temp_dir().join(format!("inca-config-absent-{}", std::process::id()));
    drop(fs::remove_dir_all(&dir));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("bundle.js"), "").unwrap();

    let read = config::read(&dir.join("bundle.js"));
    drop(fs::remove_dir_all(&dir));

    assert_eq!(read, AppConfig::default());
}

#[test]
fn the_emit_script_is_where_this_test_looks_for_it() {
    assert!(Path::new(EMIT_SCRIPT).is_file(), "{EMIT_SCRIPT}");
}
