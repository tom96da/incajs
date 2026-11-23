// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The namespace constants, the re-exports of `test_support`, and the helpers that activate a
//! window or build a window over a focusable node on a fake script.

#![allow(clippy::unwrap_used)]

use std::rc::Rc;

use gpui::{AppContext as _, TestAppContext};
use inca_node::NodeId;

use super::fake_script::{FakeScript, fake_dispatcher};
pub(crate) use crate::test_support::apply_focus;
pub(crate) use crate::test_support::{
    Frame, add_sheet, at, draw, is_focused, mouse_at, node_under_root, state, tree_of_three,
    wheel_at,
};
use crate::{Cells, EventDispatcher};

/// The HTML namespace.
pub(crate) const HTML: &str = "http://www.w3.org/1999/xhtml";

/// The SVG namespace.
pub(crate) const SVG: &str = "http://www.w3.org/2000/svg";

/// The `MathML` namespace.
pub(crate) const MATHML: &str = "http://www.w3.org/1998/Math/MathML";

pub(crate) fn set_active(
    cx: &mut TestAppContext,
    window: gpui::WindowHandle<Frame>,
    dispatcher: &EventDispatcher,
    active: bool,
) {
    cx.update_window(window.into(), |_, window, cx| {
        dispatcher.window_activated(active, window, cx);
    })
    .unwrap();
}

/// A focusable node under the root of `cells`, on which no script listens.
pub(crate) fn fake_focusable_node(cells: &Rc<Cells>) -> NodeId {
    let mut document = cells.document.borrow_mut();
    let node = document.create_node("div").unwrap();
    let root = document.root();
    document
        .append_child(&*cells.window.borrow(), root, node)
        .unwrap();
    document
        .set_attribute(node, "tabindex", inca_node::AttributeValue::from("0"))
        .unwrap();
    node
}

/// A window over one focusable node, on a [`FakeScript`].
pub(crate) fn fake_focus_scene(
    cx: &mut TestAppContext,
) -> (
    Rc<EventDispatcher>,
    Rc<Cells>,
    Rc<FakeScript>,
    gpui::WindowHandle<Frame>,
    NodeId,
) {
    let (dispatcher, cells, fake) = fake_dispatcher();
    let node = fake_focusable_node(&cells);
    let dispatcher = Rc::new(dispatcher);
    let window = cx.add_window(|_, _| Frame(Rc::clone(&dispatcher)));
    (dispatcher, cells, fake, window, node)
}
