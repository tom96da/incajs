// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Exception, Result as JsResult};

use inca_gpui::{NodeId, check_selector, closest_selector, matches_selector};

use super::Natives;
use super::convert::{JsNodeId, existing_node};
use super::host::Host;

/// `matches`, `closest` and `checkSelector`: selector matching over the retained tree.
pub(super) fn install_selector(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("matches", matches)?;
    n.set("closest", closest)?;
    n.set("checkSelector", check)
}

fn check<'js>(_host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, String) -> JsResult<()> {
    move |ctx, selector| {
        check_selector(&selector)
            .map_err(|err| Exception::throw_dom(&ctx, "SyntaxError", &err.to_string()))
    }
}

fn matches<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<bool> {
    move |ctx, JsNodeId(node_id), selector| {
        let host = host.borrow();
        existing_node(&ctx, &host.tree, node_id)?;
        matches_selector(&host.tree, node_id, &selector)
            .map_err(|err| Exception::throw_dom(&ctx, "SyntaxError", &err.to_string()))
    }
}

fn closest<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<Option<NodeId>> {
    move |ctx, JsNodeId(node_id), selector| {
        let host = host.borrow();
        existing_node(&ctx, &host.tree, node_id)?;
        closest_selector(&host.tree, node_id, &selector)
            .map_err(|err| Exception::throw_dom(&ctx, "SyntaxError", &err.to_string()))
    }
}

#[cfg(test)]
mod tests;
