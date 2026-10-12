// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;
use gpui::TestAppContext;
use inca_gpui::EventPayload;

use crate::dev::Writer;

/// The entry path these tests evaluate `source` under. Never read from
/// disk in a test that imports nothing else, since [`Session::load`]
/// only reads a sibling file when a `source` actually imports one.
const TEST_ENTRY_PATH: &str = "/test/entry.js";

fn load(source: &str) -> Result<Session, EngineError> {
    Session::load(TEST_ENTRY_PATH, source, stderr_reporter())
}

mod load;
mod report;
mod window;

const DEFERS_ITS_MOUNT: &str = r"
    globalThis.mounted = false;
    Promise.resolve().then(() => { globalThis.mounted = true; });
";
