// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use inca_node::NodeId;
use rquickjs::{Ctx, IntoJs, Result as JsResult, Value};

/// The JS tag of [`EventTargetId::Window`].
pub(crate) const WINDOW_TAG: &str = "$window";

/// Something an event can be dispatched at or travel through: a node of the
/// tree, or the tag `"$window"`, which is no node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventTargetId {
    Node(NodeId),
    Window,
}

/// A node is its number, and a global target is its tag.
impl<'js> IntoJs<'js> for EventTargetId {
    fn into_js(self, ctx: &Ctx<'js>) -> JsResult<Value<'js>> {
        match self {
            Self::Node(id) => id.into_js(ctx),
            Self::Window => WINDOW_TAG.into_js(ctx),
        }
    }
}
