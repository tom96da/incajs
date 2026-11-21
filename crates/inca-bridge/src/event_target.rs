// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use inca_document::EventTargetId;
use inca_node::NodeId;
use rquickjs::{Ctx, IntoJs, Result as JsResult, Value};

/// The JS tag of [`HookTarget::Window`].
pub(crate) const WINDOW_TAG: &str = "$window";

/// A target the global `__inca_targets__` gives an object for: a node or the window. A signal
/// is no hook target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HookTarget {
    Node(NodeId),
    Window,
}

impl HookTarget {
    /// The hook target of `target`, `None` for a signal.
    pub(crate) fn of(target: EventTargetId) -> Option<Self> {
        match target {
            EventTargetId::Node(id) => Some(Self::Node(id)),
            EventTargetId::Window => Some(Self::Window),
            EventTargetId::Signal(_) => None,
        }
    }
}

/// A node is its number, and the window is its tag.
impl<'js> IntoJs<'js> for HookTarget {
    fn into_js(self, ctx: &Ctx<'js>) -> JsResult<Value<'js>> {
        match self {
            Self::Node(id) => id.into_js(ctx),
            Self::Window => WINDOW_TAG.into_js(ctx),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use inca_jsenv::Engine;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::node(EventTargetId::Node(7), Some(HookTarget::Node(7)))]
    #[case::window(EventTargetId::Window, Some(HookTarget::Window))]
    #[case::signal(EventTargetId::Signal(3), None)]
    fn the_hook_takes_nodes_and_the_window_only(
        #[case] target: EventTargetId,
        #[case] hook: Option<HookTarget>,
    ) {
        assert_eq!(HookTarget::of(target), hook);
    }

    #[rstest]
    #[case::node(HookTarget::Node(7), "7")]
    #[case::window(HookTarget::Window, "\"$window\"")]
    fn a_hook_target_reaches_js_as_a_number_or_its_tag(
        #[case] target: HookTarget,
        #[case] json: &str,
    ) {
        let engine = Engine::new().unwrap();

        let text = engine.with(|ctx| {
            let value = target.into_js(&ctx).unwrap();
            ctx.json_stringify(value)
                .unwrap()
                .unwrap()
                .to_string()
                .unwrap()
        });

        assert_eq!(text, json);
    }
}
