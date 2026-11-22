// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Manual example for the full JS → native tree → GPUI element round trip:
//! a real `QuickJS` engine drives a `VirtualTree` through `__inca_native__`,
//! and clicking the box calls a real JS callback that updates the label
//! through `__inca_native__.setCharacterData`. Needs a human to look at it:
//! click the box and the label should count up.

#![allow(clippy::unwrap_used)]

use std::rc::Rc;

use gpui::{App, Bounds, Context, Window, WindowBounds, WindowOptions, prelude::*, px, size};
use gpui_platform::application;

use inca_bridge::{Host, JsScript, install};
use inca_gpui::{Scrollbars, render_tree_with_events};
use inca_host::script::{BridgeScript, compose};
use inca_jsenv::{Engine, stderr_reporter, system_clock};
use inca_node::{NodeId, VirtualTree};
use inca_style::set_style;
use inca_window::EventDispatcher;

struct ClickCounter {
    host: Rc<Host>,
    root: NodeId,
    dispatcher: EventDispatcher,
}

impl Render for ClickCounter {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let host = self.host.document.borrow();
        render_tree_with_events(
            &host.tree,
            &host.style,
            self.root,
            &self.dispatcher,
            Scrollbars::Overlay,
        )
        .unwrap()
    }
}

/// Builds a clickable, bordered box containing a text label and returns
/// `(outer, label)`.
fn build_tree(tree: &mut VirtualTree) -> (NodeId, NodeId) {
    let outer = tree.create_node("div").unwrap();
    set_style(tree, outer, "display", "flex").unwrap();
    set_style(tree, outer, "justify_content", "center").unwrap();
    set_style(tree, outer, "align_items", "center").unwrap();
    set_style(tree, outer, "width", 300.0).unwrap();
    set_style(tree, outer, "height", 150.0).unwrap();
    set_style(tree, outer, "background", f64::from(0x505050)).unwrap();
    set_style(tree, outer, "border_width", 1.0).unwrap();
    set_style(tree, outer, "border_color", f64::from(0x0000ff)).unwrap();
    set_style(tree, outer, "corner_radius", 8.0).unwrap();
    set_style(tree, outer, "text_color", f64::from(0xffffff)).unwrap();
    set_style(tree, outer, "text_size", 20.0).unwrap();

    let label = tree.create_node("#text").unwrap();
    tree.set_data(label, "Click me!").unwrap();
    tree.append_child(outer, label).unwrap();

    (outer, label)
}

/// Wires the box's click, via JS, to update the label's text. Passes the callback
/// function to `addEventListener` (see `JsScript`'s doc comment for why).
fn install_click_handler(engine: &Engine, outer: NodeId, label: NodeId) {
    engine
        .eval::<()>(&format!(
            r"
            globalThis.__inca_clicks__ = 0;
            __inca_native__.addEventListener({outer}, 'click', function () {{
                globalThis.__inca_clicks__ += 1;
                __inca_native__.setCharacterData(
                    {label},
                    'Clicked ' + globalThis.__inca_clicks__ + ' time(s)'
                );
            }});
            "
        ))
        .unwrap();
}

fn run_example() {
    application().run(|cx: &mut App| {
        let (host, cells) = compose(system_clock());
        let (outer, label) = build_tree(&mut host.document.borrow_mut().tree);

        let engine = Rc::new(Engine::new().unwrap());
        engine.with(|ctx| install(&ctx, &host)).unwrap();
        install_click_handler(&engine, outer, label);
        let script = JsScript::new(engine, Rc::clone(&host), stderr_reporter());
        let dispatcher = EventDispatcher::new(Rc::new(BridgeScript(script)), cells);

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
