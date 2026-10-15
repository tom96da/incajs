// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Helpers shared by the tests of several modules.

#![allow(clippy::unwrap_used)]

use std::{
    env, fs,
    ops::Deref,
    path::{Path, PathBuf},
};

/// A directory under the OS temp root, unique per process, test name and
/// thread, removed on drop.
pub(crate) struct ScratchDir(PathBuf);

impl ScratchDir {
    pub(crate) fn new(name: &str) -> Self {
        let path = env::temp_dir().join(format!(
            "inca-host-test-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Deref for ScratchDir {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
