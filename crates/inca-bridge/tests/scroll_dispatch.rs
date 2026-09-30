// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration tests for wheel scrolling of scroll containers: a real
//! `VirtualTree` rendered through `render_tree_with_events`, scrolled by
//! `ScrollWheelEvent`s.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    Context, Pixels, Point, Render, ScrollDelta, ScrollWheelEvent, TestAppContext,
    VisualTestContext, Window, point, prelude::*, px,
};
use inca_bridge::{EventDispatcher, Host};
use inca_gpui::{EventSink, NodeId, render_tree_with_events};
use inca_jsenv::{Engine, EngineError};

struct Root {
    host: Rc<RefCell<Host>>,
    dispatcher: EventDispatcher,
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let host = self.host.borrow();
        render_tree_with_events(&host.tree, 0, &self.dispatcher).unwrap()
    }
}

struct Scene {
    engine: Rc<Engine>,
    dispatcher: EventDispatcher,
    cx: VisualTestContext,
    reported: Rc<RefCell<Vec<EngineError>>>,
}

impl Scene {
    /// Renders node 0 of `host` with `js` as the `__inca_callbacks__` body.
    fn new(cx: &mut TestAppContext, host: Host, js: &str) -> Self {
        let host = Rc::new(RefCell::new(host));
        let engine = Rc::new(Engine::new().unwrap());
        engine
            .eval::<()>(&format!(
                "globalThis.ran = []; globalThis.__inca_callbacks__ = {{ {js} }};"
            ))
            .unwrap();
        let reported = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&reported);
        let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host))
            .with_reporter(Rc::new(move |err| sink.borrow_mut().push(err.clone())));
        let window = cx.add_window(|_, _| Root {
            host,
            dispatcher: dispatcher.clone(),
        });
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
        Self {
            engine,
            dispatcher,
            cx: VisualTestContext::from_window(window.into(), cx),
            reported,
        }
    }

    /// One wheel event over the top-left corner; a negative `y` scrolls down.
    fn wheel(&mut self, y: f32) {
        self.wheel_at(10., y);
    }

    /// Like [`Self::wheel`], with the pointer `pointer_y` from the top.
    fn wheel_at(&mut self, pointer_y: f32, y: f32) {
        self.wheel_delta(pointer_y, ScrollDelta::Pixels(point(px(0.), px(y))));
    }

    /// One wheel event with `delta`, then a draw so offsets are clamped.
    fn wheel_delta(&mut self, pointer_y: f32, delta: ScrollDelta) {
        self.cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(pointer_y)),
            delta,
            ..Default::default()
        });
        self.cx.run_until_parked();
        self.cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    /// Runs `code` in the app's JS context.
    fn js(&self, code: &str) {
        self.engine.eval::<()>(code).unwrap();
    }

    fn offset(&self, node: NodeId) -> Point<Pixels> {
        self.dispatcher.scroll_handle(node).unwrap().offset()
    }

    fn line_height(&mut self) -> Pixels {
        self.cx.update(|window, _| window.line_height())
    }

    fn ran(&self) -> String {
        self.engine.eval("JSON.stringify(ran)").unwrap()
    }
}

/// Adds a sized div under `parent`, or styles the root when there is none.
fn add_div(host: &mut Host, parent: Option<NodeId>, height: f64, scrolls: bool) -> NodeId {
    let node = if parent.is_some() {
        host.tree.create_node("div")
    } else {
        host.root
    };
    host.tree.set_style(node, "width", 100.0).unwrap();
    host.tree.set_style(node, "height", height).unwrap();
    if scrolls {
        host.tree.set_style(node, "overflow_y", "scroll").unwrap();
    }
    if let Some(parent) = parent {
        host.tree.append_child(parent, node).unwrap();
    }
    node
}

/// Node 0 scrolls (100px over 300px of content, node 1).
fn one_container() -> Host {
    let mut host = Host::default();
    let container = add_div(&mut host, None, 100.0, true);
    add_div(&mut host, Some(container), 300.0, false);
    host
}

/// Node 0 scrolls 200px over node 1, which scrolls 200px over node 2, and
/// node 3 fills the rest of node 0.
fn nested_containers() -> Host {
    let mut host = Host::default();
    let outer = add_div(&mut host, None, 100.0, true);
    let inner = add_div(&mut host, Some(outer), 100.0, true);
    add_div(&mut host, Some(inner), 300.0, false);
    add_div(&mut host, Some(outer), 200.0, false);
    host
}

/// A container of 100x100 with `styles` over a 300x300 child.
fn styled_container(styles: &[(&str, &str)]) -> Host {
    let mut host = Host::default();
    let root = host.root;
    host.tree.set_style(root, "width", 100.0).unwrap();
    host.tree.set_style(root, "height", 100.0).unwrap();
    for (key, value) in styles {
        host.tree.set_style(root, *key, *value).unwrap();
    }
    let child = host.tree.create_node("div");
    host.tree.set_style(child, "width", 300.0).unwrap();
    host.tree.set_style(child, "height", 300.0).unwrap();
    host.tree.append_child(root, child).unwrap();
    host
}

/// Three scroll containers, each 100px over 200px of further content: node 0
/// holds node 1, which holds node 2, which holds node 3 (300px).
fn triple_containers() -> Host {
    let mut host = Host::default();
    let outer = add_div(&mut host, None, 100.0, true);
    let middle = add_div(&mut host, Some(outer), 100.0, true);
    let inner = add_div(&mut host, Some(middle), 100.0, true);
    add_div(&mut host, Some(inner), 300.0, false);
    add_div(&mut host, Some(middle), 200.0, false);
    add_div(&mut host, Some(outer), 200.0, false);
    host
}

/// Callbacks by id, each recording its name in `ran`.
const RECORD_WHEEL: &str = concat!(
    // container: records only
    "0: () => { ran.push('container'); }, ",
    // child: stops propagation
    "1: (e) => { ran.push('child'); e.stopPropagation(); }, ",
    // child: prevents default
    "2: (e) => { ran.push('child'); e.preventDefault(); }, ",
    // container: prevents default
    "3: (e) => { ran.push('container'); e.preventDefault(); }, ",
    // a second listener on the same node
    "4: () => { ran.push('second'); }, ",
    // stops immediately
    "5: (e) => { ran.push('immediate'); e.stopImmediatePropagation(); }, ",
    // container: stops propagation
    "6: (e) => { ran.push('container'); e.stopPropagation(); }, ",
    // prevents default while `globalThis.prevent` is set
    "7: (e) => { ran.push('prevent'); if (globalThis.prevent) e.preventDefault(); }, ",
    // child: stops propagation while `globalThis.stop` is set
    "8: (e) => { ran.push('child'); if (globalThis.stop) e.stopPropagation(); }, ",
    // child: stops propagation, then throws, while `globalThis.stop` is set
    "9: (e) => { ran.push('child'); if (globalThis.stop) { e.stopPropagation(); throw new Error('boom'); } }, ",
    // child: stops propagation and prevents default
    "10: (e) => { ran.push('child'); e.stopPropagation(); e.preventDefault(); }",
);

#[gpui::test]
fn a_container_scrolls(cx: &mut TestAppContext) {
    let mut scene = Scene::new(cx, one_container(), "");
    scene.wheel(-50.);
    assert_eq!(scene.offset(0).y, px(-50.));
}

#[gpui::test]
fn stop_propagation_skips_ancestors_but_not_the_scroll(cx: &mut TestAppContext) {
    let mut host = one_container();
    host.listeners.register(0, "wheel", 0);
    host.listeners.register(1, "wheel", 1);
    let mut scene = Scene::new(cx, host, RECORD_WHEEL);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["child"]"#);
    assert_eq!(scene.offset(0).y, px(-50.));

    scene.wheel(-20.);
    assert_eq!(
        scene.ran(),
        r#"["child","child"]"#,
        "the stop ends with its event"
    );
    assert_eq!(scene.offset(0).y, px(-70.));
}

#[gpui::test]
fn prevent_default_from_a_child_cancels_the_scroll(cx: &mut TestAppContext) {
    let mut host = one_container();
    host.listeners.register(1, "wheel", 2);
    let mut scene = Scene::new(cx, host, RECORD_WHEEL);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["child"]"#);
    assert_eq!(scene.offset(0).y, px(0.));
}

#[gpui::test]
fn prevent_default_from_the_container_itself_cancels_the_scroll(cx: &mut TestAppContext) {
    let mut host = one_container();
    host.listeners.register(0, "wheel", 3);
    let mut scene = Scene::new(cx, host, RECORD_WHEEL);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["container"]"#);
    assert_eq!(scene.offset(0).y, px(0.));
}

#[gpui::test]
fn only_the_innermost_container_scrolls(cx: &mut TestAppContext) {
    let mut scene = Scene::new(cx, nested_containers(), "");
    scene.wheel(-50.);
    assert_eq!(scene.offset(1).y, px(-50.));
    assert_eq!(scene.offset(0).y, px(0.));
}

#[gpui::test]
fn an_inner_container_at_its_limit_chains_to_the_outer(cx: &mut TestAppContext) {
    let mut scene = Scene::new(cx, nested_containers(), "");
    scene.wheel(-200.);
    assert_eq!(scene.offset(1).y, px(-200.));
    assert_eq!(scene.offset(0).y, px(0.));

    scene.wheel(-50.);
    assert_eq!(scene.offset(1).y, px(-200.));
    assert_eq!(scene.offset(0).y, px(-50.));
}

#[gpui::test]
fn a_cancelled_scroll_leaves_the_bounds_unmoved(cx: &mut TestAppContext) {
    let mut host = one_container();
    host.listeners.register(1, "wheel", 2);
    let mut scene = Scene::new(cx, host, RECORD_WHEEL);
    let before = scene.cx.debug_bounds("node-1").unwrap();
    scene.wheel(-50.);
    assert_eq!(scene.cx.debug_bounds("node-1").unwrap(), before);
}

#[gpui::test]
fn a_wheel_over_no_container_leaves_every_offset_alone(cx: &mut TestAppContext) {
    let mut host = Host::default();
    let root = add_div(&mut host, None, 250.0, false);
    for _ in 0..2 {
        let sibling = add_div(&mut host, Some(root), 100.0, true);
        add_div(&mut host, Some(sibling), 300.0, false);
    }
    let mut scene = Scene::new(cx, host, "");
    scene.wheel(-50.);
    scene.wheel_at(220., -50.);
    assert_eq!(scene.offset(1).y, px(-50.));
    assert_eq!(scene.offset(3).y, px(0.));
}

#[gpui::test]
fn only_the_container_under_the_pointer_scrolls(cx: &mut TestAppContext) {
    let mut host = Host::default();
    let root = add_div(&mut host, None, 200.0, false);
    for _ in 0..2 {
        let sibling = add_div(&mut host, Some(root), 100.0, true);
        add_div(&mut host, Some(sibling), 300.0, false);
    }
    let mut scene = Scene::new(cx, host, "");
    scene.wheel(-50.);
    assert_eq!(scene.offset(1).y, px(-50.));
    assert_eq!(scene.offset(3).y, px(0.));

    scene.wheel_at(110., -30.);
    assert_eq!(scene.offset(1).y, px(-50.));
    assert_eq!(scene.offset(3).y, px(-30.));
}

#[gpui::test]
fn prevent_default_from_the_inner_container_cancels_both_scrolls(cx: &mut TestAppContext) {
    let mut host = nested_containers();
    host.listeners.register(1, "wheel", 2);
    let mut scene = Scene::new(cx, host, RECORD_WHEEL);
    scene.wheel(-50.);
    scene.wheel(-250.);
    assert_eq!(scene.offset(1).y, px(0.));
    assert_eq!(scene.offset(0).y, px(0.));
}

#[gpui::test]
fn stop_propagation_from_the_container_itself_keeps_its_other_listeners(cx: &mut TestAppContext) {
    let mut host = one_container();
    host.listeners.register(0, "wheel", 6);
    host.listeners.register(0, "wheel", 4);
    let mut scene = Scene::new(cx, host, RECORD_WHEEL);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["container","second"]"#);
    assert_eq!(scene.offset(0).y, px(-50.));
}

#[gpui::test]
fn stop_immediate_propagation_from_a_child_still_scrolls(cx: &mut TestAppContext) {
    let mut host = one_container();
    host.listeners.register(0, "wheel", 0);
    host.listeners.register(1, "wheel", 5);
    host.listeners.register(1, "wheel", 4);
    let mut scene = Scene::new(cx, host, RECORD_WHEEL);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["immediate"]"#);
    assert_eq!(scene.offset(0).y, px(-50.));
}

#[gpui::test]
fn a_stopped_wheel_does_not_silence_the_next_one(cx: &mut TestAppContext) {
    let mut host = one_container();
    host.listeners.register(0, "wheel", 0);
    host.listeners.register(1, "wheel", 8);
    let mut scene = Scene::new(cx, host, RECORD_WHEEL);
    scene.js("globalThis.stop = true;");
    scene.wheel(-10.);
    scene.js("globalThis.stop = false;");
    scene.wheel(-10.);
    assert_eq!(scene.ran(), r#"["child","child","container"]"#);
    assert_eq!(scene.offset(0).y, px(-20.));
}

#[gpui::test]
fn repeated_wheels_add_up_and_stop_at_the_end(cx: &mut TestAppContext) {
    let mut scene = Scene::new(cx, one_container(), "");
    scene.wheel(-50.);
    scene.wheel(-50.);
    assert_eq!(scene.offset(0).y, px(-100.));
    scene.wheel(-300.);
    assert_eq!(scene.offset(0).y, px(-200.));
    scene.wheel(-50.);
    assert_eq!(scene.offset(0).y, px(-200.));
}

#[gpui::test]
fn scrolling_up_at_the_top_moves_nothing(cx: &mut TestAppContext) {
    let mut scene = Scene::new(cx, one_container(), "");
    scene.wheel(50.);
    assert_eq!(scene.offset(0).y, px(0.));
}

#[gpui::test]
fn scrolling_up_chains_outward_from_an_inner_container_at_the_top(cx: &mut TestAppContext) {
    let mut host = Host::default();
    let outer = add_div(&mut host, None, 150.0, true);
    let inner = add_div(&mut host, Some(outer), 100.0, true);
    add_div(&mut host, Some(inner), 300.0, false);
    add_div(&mut host, Some(outer), 200.0, false);
    let mut scene = Scene::new(cx, host, "");
    scene.wheel_at(120., -50.);
    assert_eq!(scene.offset(0).y, px(-50.));

    scene.wheel(30.);
    assert_eq!(scene.offset(1).y, px(0.));
    assert_eq!(scene.offset(0).y, px(-20.));
}

#[gpui::test]
fn three_nested_containers_chain_from_the_inside_out(cx: &mut TestAppContext) {
    let mut scene = Scene::new(cx, triple_containers(), "");
    let offsets = |scene: &Scene| [2, 1, 0].map(|node| scene.offset(node).y);

    scene.wheel(-200.);
    assert_eq!(offsets(&scene), [px(-200.), px(0.), px(0.)]);
    scene.wheel(-50.);
    assert_eq!(offsets(&scene), [px(-200.), px(-50.), px(0.)]);
    scene.wheel(-150.);
    assert_eq!(offsets(&scene), [px(-200.), px(-200.), px(0.)]);
    scene.wheel(-50.);
    assert_eq!(offsets(&scene), [px(-200.), px(-200.), px(-50.)]);
}

#[gpui::test]
fn prevent_default_from_an_ancestor_cancels_the_scroll(cx: &mut TestAppContext) {
    let mut host = Host::default();
    let root = add_div(&mut host, None, 100.0, false);
    let container = add_div(&mut host, Some(root), 100.0, true);
    add_div(&mut host, Some(container), 300.0, false);
    host.listeners.register(root, "wheel", 2);
    let mut scene = Scene::new(cx, host, RECORD_WHEEL);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["child"]"#);
    assert_eq!(scene.offset(container).y, px(0.));
}

#[gpui::test]
fn a_hidden_container_does_not_scroll(cx: &mut TestAppContext) {
    let mut scene = Scene::new(cx, styled_container(&[("overflow", "hidden")]), "");
    let before = scene.cx.debug_bounds("node-1").unwrap();
    scene.wheel(-50.);
    assert_eq!(scene.cx.debug_bounds("node-1").unwrap(), before);
}

#[gpui::test]
fn a_diagonal_wheel_scrolls_only_the_scrolling_axis(cx: &mut TestAppContext) {
    let host = styled_container(&[("overflow_x", "hidden"), ("overflow_y", "scroll")]);
    let mut scene = Scene::new(cx, host, "");
    scene.wheel_delta(10., ScrollDelta::Pixels(point(px(-10.), px(-50.))));
    assert_eq!(scene.offset(0), point(px(0.), px(-50.)));
}

// Pins gpui's axis rule (B-082): the y delta moves the container even when
// the x delta is larger.
#[gpui::test]
fn an_x_dominant_wheel_moves_a_y_only_container_by_its_y_delta(cx: &mut TestAppContext) {
    let host = styled_container(&[("overflow_x", "hidden"), ("overflow_y", "scroll")]);
    let mut scene = Scene::new(cx, host, "");
    scene.wheel_delta(10., ScrollDelta::Pixels(point(px(-50.), px(-10.))));
    assert_eq!(scene.offset(0), point(px(0.), px(-10.)));
}

#[gpui::test]
fn a_horizontal_wheel_scrolls_an_x_only_container(cx: &mut TestAppContext) {
    let mut scene = Scene::new(cx, styled_container(&[("overflow_x", "scroll")]), "");
    scene.wheel_delta(10., ScrollDelta::Pixels(point(px(-50.), px(0.))));
    assert_eq!(scene.offset(0), point(px(-50.), px(0.)));
}

// Pins gpui's axis rule (B-082).
#[gpui::test]
fn a_vertical_wheel_scrolls_an_x_only_container_along_x(cx: &mut TestAppContext) {
    let mut scene = Scene::new(cx, styled_container(&[("overflow_x", "scroll")]), "");
    scene.wheel(-50.);
    assert_eq!(scene.offset(0), point(px(-50.), px(0.)));
}

#[gpui::test]
fn a_line_delta_scrolls_by_the_line_height(cx: &mut TestAppContext) {
    let mut scene = Scene::new(cx, one_container(), "");
    scene.wheel_delta(10., ScrollDelta::Lines(point(0., -2.)));
    assert_eq!(scene.offset(0).y, scene.line_height() * -2.);
}

#[gpui::test]
fn a_wheel_after_a_prevented_one_scrolls_again(cx: &mut TestAppContext) {
    let mut host = one_container();
    host.listeners.register(0, "wheel", 7);
    let mut scene = Scene::new(cx, host, RECORD_WHEEL);
    scene.js("globalThis.prevent = true;");
    scene.wheel(-50.);
    assert_eq!(scene.offset(0).y, px(0.));

    scene.js("globalThis.prevent = false;");
    scene.wheel(-50.);
    assert_eq!(scene.offset(0).y, px(-50.));
}

#[gpui::test]
fn listeners_run_child_first_and_the_container_still_scrolls(cx: &mut TestAppContext) {
    let mut host = one_container();
    host.listeners.register(0, "wheel", 0);
    host.listeners.register(1, "wheel", 8);
    let mut scene = Scene::new(cx, host, RECORD_WHEEL);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["child","container"]"#);
    assert_eq!(scene.offset(0).y, px(-50.));
}

#[gpui::test]
fn a_throw_after_stop_propagation_still_stops_ancestors_and_scrolls(cx: &mut TestAppContext) {
    let mut host = one_container();
    host.listeners.register(0, "wheel", 0);
    host.listeners.register(1, "wheel", 9);
    let mut scene = Scene::new(cx, host, RECORD_WHEEL);
    scene.js("globalThis.stop = true;");
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["child"]"#);
    assert_eq!(scene.offset(0).y, px(-50.));
    assert_eq!(scene.reported.borrow().len(), 1);
    assert_eq!(scene.reported.borrow()[0].message(), "Error: boom");

    scene.js("globalThis.stop = false;");
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["child","child","container"]"#);
    assert_eq!(scene.offset(0).y, px(-100.));
    assert_eq!(scene.reported.borrow().len(), 1);
}

#[gpui::test]
fn stop_propagation_with_prevent_default_skips_ancestors_and_cancels(cx: &mut TestAppContext) {
    let mut host = one_container();
    host.listeners.register(0, "wheel", 0);
    host.listeners.register(1, "wheel", 10);
    let mut scene = Scene::new(cx, host, RECORD_WHEEL);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["child"]"#);
    assert_eq!(scene.offset(0).y, px(0.));
}
