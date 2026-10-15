// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Helpers shared by the tests of several modules.

#![allow(clippy::unwrap_used)]

use std::{
    env, fs,
    ops::Deref,
    path::{Path, PathBuf},
};

use rquickjs::{Context, Ctx, Runtime};

use crate::paint::Style;

/// Runs `body` in a fresh runtime with a full context.
pub(crate) fn with_realm<R>(body: impl FnOnce(Ctx<'_>) -> R) -> R {
    let runtime = Runtime::new().unwrap();
    let context = Context::full(&runtime).unwrap();
    context.with(body)
}

/// A directory under the OS temp root, unique per process, test name and
/// thread, removed on drop.
pub(crate) struct ScratchDir(pub(crate) PathBuf);

impl ScratchDir {
    pub(crate) fn new(name: &str) -> Self {
        let path = env::temp_dir().join(format!(
            "inca-jsenv-test-{name}-{}-{:?}",
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

/// `text` without the sequences of every [`Style`]. Other escapes stay, so a
/// stray one is still visible.
pub(crate) fn strip(text: &str) -> String {
    let mut out = text.to_owned();
    for style in [
        Style::Red,
        Style::Green,
        Style::Yellow,
        Style::Blue,
        Style::Magenta,
        Style::Cyan,
        Style::Bold,
        Style::Grey,
    ] {
        let (open, close) = style.codes();
        out = out
            .replace(&format!("\x1b[{open}m"), "")
            .replace(&format!("\x1b[{close}m"), "");
    }
    out
}
