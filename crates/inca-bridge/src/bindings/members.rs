// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The members the host defines on the stored prototypes of the node interfaces. Each takes its
//! receiver through the rules of `handle.rs`.

mod element;

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Result as JsResult};

use super::host::Host;

/// Defines the host members on the prototypes `interfaces::install` stored.
pub(super) fn install_members(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>) -> JsResult<()> {
    element::install(ctx, host)
}
