// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Manual example for the full JS → native tree → GPUI element round trip:
//! a real `QuickJS` engine drives a `VirtualTree` through `__inca_native__`,
//! and clicking the box calls a real JS callback that updates the label
//! through `__inca_native__.setAttribute`. Needs a human to look at it:
//! click the box and the label should count up.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{App, Bounds, Context, Window, WindowBounds, WindowOptions, prelude::*, px, size};
use gpui_platform::application;

use inca_bridge::bindings::install;
use inca_bridge::{EventDispatcher, Host};
use inca_gpui::{NodeId, VirtualTree, render_tree_with_events};
use inca_jsenv::Engine;

struct ClickCounter {
    host: Rc<RefCell<Host>>,
    root: NodeId,
    dispatcher: EventDispatcher,
}

impl Render for ClickCounter {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let host = self.host.borrow();
        render_tree_with_events(&host.tree, self.root, &self.dispatcher).unwrap()
    }
}

/// Builds a clickable, bordered box containing a text label and returns
/// `(outer, label)`.
fn build_tree(tree: &mut VirtualTree) -> (NodeId, NodeId) {
    let outer = tree.create_node("div");
    tree.set_style(outer, "display", "flex").unwrap();
    tree.set_style(outer, "justify_content", "center").unwrap();
    tree.set_style(outer, "align_items", "center").unwrap();
    tree.set_style(outer, "width", 300.0).unwrap();
    tree.set_style(outer, "height", 150.0).unwrap();
    tree.set_style(outer, "background", f64::from(0x505050))
        .unwrap();
    tree.set_style(outer, "border_width", 1.0).unwrap();
    tree.set_style(outer, "border_color", f64::from(0x0000ff))
        .unwrap();
    tree.set_style(outer, "corner_radius", 8.0).unwrap();
    tree.set_style(outer, "text_color", f64::from(0xffffff))
        .unwrap();
    tree.set_style(outer, "text_size", 20.0).unwrap();

    let label = tree.create_node("text");
    tree.set_attribute(label, "value", "Click me!").unwrap();
    tree.append_child(outer, label).unwrap();

    (outer, label)
}

/// Wires the box's click, via JS, to update the label's text. Relies on the
/// convention that a real callback function must be stored at
/// `globalThis.__inca_callbacks__[callbackId]` before calling
/// `addEventListener` with that id (see `render::bridge::EventDispatcher`'s
/// doc comment for why).
fn install_click_handler(engine: &Engine, outer: NodeId, label: NodeId) {
    engine
        .eval::<()>(&format!(
            "globalThis.__inca_clicks__ = 0;
             globalThis.__inca_callbacks__ = {{
                 0: function () {{
                     globalThis.__inca_clicks__ += 1;
                     __inca_native__.setAttribute(
                         {label},
                         'value',
                         'Clicked ' + globalThis.__inca_clicks__ + ' time(s)'
                     );
                 }}
             }};
             __inca_native__.addEventListener({outer}, 'click', 0);"
        ))
        .unwrap();
}

fn run_example() {
    application().run(|cx: &mut App| {
        let host = Rc::new(RefCell::new(Host::default()));
        let (outer, label) = build_tree(&mut host.borrow_mut().tree);

        let engine = Engine::new().unwrap();
        engine.with(|ctx| install(&ctx, &host)).unwrap();
        install_click_handler(&engine, outer, label);
        let dispatcher = EventDispatcher::new(Rc::new(engine), Rc::clone(&host));

        let bounds = Bounds::centered(None, size(px(300.), px(150.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| ClickCounter {
                    host,
                    root: outer,
                    dispatcher,
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}

fn main() {
    env_logger::init();
    run_example();
}
