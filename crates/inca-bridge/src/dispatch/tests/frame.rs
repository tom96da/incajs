// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;
use gpui::{AppContext as _, Context};

const SEEN: &str = "globalThis.seen = []; \
    globalThis.__inca_callbacks__ = { \
        0: (e) => { globalThis.seen.push(e.type + ':' + e.target); } \
    };";

fn focusable_node(dispatcher: &EventDispatcher, host: &Rc<RefCell<Host>>) -> NodeId {
    let mut host = host.borrow_mut();
    let node = host.tree.create_node("div").unwrap();
    host.tree
        .set_attribute(node, "tabindex", inca_gpui::AttributeValue::from("0"))
        .unwrap();
    for event in ["focus", "blur"] {
        host.listeners.register(node, event, 0);
    }
    host.focus.mark_tab_dirty(node);
    dispatcher.engine.eval::<()>(SEEN).unwrap();
    node
}

/// A view whose render runs `begin_frame`, as a window's render does.
struct Frame(Rc<EventDispatcher>);

impl gpui::Render for Frame {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        self.0.begin_frame(window, cx);
        gpui::div()
    }
}

fn draw(cx: &mut TestAppContext, window: gpui::WindowHandle<Frame>) {
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
}

fn seen(dispatcher: &EventDispatcher) -> String {
    dispatcher
        .engine
        .eval::<String>("JSON.stringify(globalThis.seen)")
        .unwrap()
}

#[gpui::test]
fn begin_frame_applies_a_requested_focus_and_fires_its_events(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = focusable_node(&dispatcher, &host);
    let dispatcher = Rc::new(dispatcher);
    let window = cx.add_window(|_, _| Frame(Rc::clone(&dispatcher)));
    draw(cx, window);
    assert_eq!(seen(&dispatcher), "[]");

    host.borrow_mut().focus.request_focus(node, true);
    draw(cx, window);

    assert_eq!(seen(&dispatcher), format!(r#"["focus:{node}"]"#));
}

#[gpui::test]
fn window_activated_fires_blur_when_lost_and_focus_when_regained(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = focusable_node(&dispatcher, &host);
    let dispatcher = Rc::new(dispatcher);
    let window = cx.add_window(|_, _| Frame(Rc::clone(&dispatcher)));
    host.borrow_mut().focus.request_focus(node, true);
    draw(cx, window);
    dispatcher
        .engine
        .eval::<()>("globalThis.seen = [];")
        .unwrap();

    cx.update_window(window.into(), |_, window, cx| {
        dispatcher.window_activated(false, window, cx);
    })
    .unwrap();
    assert_eq!(seen(&dispatcher), format!(r#"["blur:{node}"]"#));

    cx.update_window(window.into(), |_, window, cx| {
        dispatcher.window_activated(true, window, cx);
    })
    .unwrap();
    assert_eq!(
        seen(&dispatcher),
        format!(r#"["blur:{node}","focus:{node}"]"#)
    );
}
