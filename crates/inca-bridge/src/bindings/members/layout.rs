// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The members that read the last drawn box of an element or act on it. A node that is not
//! connected has no box. Its reads give `0`, and its scroll writes and actions do nothing.

mod actions;
mod boxes;
#[cfg(test)]
mod fixture;
mod scroll;

use std::rc::Rc;

use rquickjs::{Ctx, Result as JsResult};

use inca_jsenv::required_prototype;

use crate::bindings::host::Host;

/// Defines the layout members on the prototypes of `Element`, `HTMLElement` and the foreign
/// element.
pub(super) fn install(ctx: &Ctx<'_>, host: &Rc<Host>) -> JsResult<()> {
    let element = required_prototype(ctx, "Element")?;
    boxes::install_client(ctx, &element, host)?;
    scroll::install(ctx, &element, host)?;
    let html = required_prototype(ctx, "HTMLElement")?;
    boxes::install_offset(ctx, &html, host)?;
    actions::install(ctx, &html, host)
}
