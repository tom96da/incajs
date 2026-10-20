// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The runtime's store of source maps.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

#[allow(clippy::disallowed_types)]
use rquickjs::JsLifetime;

use super::Entry;

/// The source maps of one runtime, keyed by the filename `QuickJS` reports.
#[allow(clippy::disallowed_macros)]
#[derive(Default, JsLifetime)]
pub(super) struct SourceMaps(pub(super) RefCell<HashMap<String, Rc<Entry>>>);
