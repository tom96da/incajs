// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::float_cmp, clippy::unwrap_used)]

use super::*;
use inca_gpui::EventSink;

#[gpui::test]
fn settle_flushes_a_microtask_mount(cx: &mut TestAppContext) {
    let mut h = Harness::load(cx, TEST_ENTRY_PATH, "");
    let before = h.snapshot().children.len();
    h.window
        .read_with(&h.cx, |app, _| {
            app.session
                .engine
                .eval::<()>(
                    r"
                    Promise.resolve().then(() => {
                        const n = __inca_native__;
                        const d = n.createNode('div');
                        n.setStyle(d, 'width', 40);
                        n.setStyle(d, 'height', 20);
                        n.appendChild(n.rootNodeId(), d);
                    });
                    ",
                )
                .unwrap();
        })
        .unwrap();
    assert_eq!(h.snapshot().children.len(), before);

    h.settle();

    let snap = h.snapshot();
    assert_eq!(snap.children.len(), before + 1);
    assert_eq!(snap.children[before].bounds.unwrap().width, 40.0);
}

const SCROLLER: &str = r"
        const n = __inca_native__;
        const c = n.createNode('div');
        n.setStyle(c, 'width', 100);
        n.setStyle(c, 'height', 50);
        n.setStyle(c, 'overflow_y', 'scroll');
        n.setStyle(c, 'overflow_x', 'scroll');
        n.appendChild(n.rootNodeId(), c);
        const label = n.createNode('text');
        n.setAttribute(label, 'value', 'hi');
        n.appendChild(c, label);
        const inner = n.createNode('div');
        n.setStyle(inner, 'width', 300);
        n.setStyle(inner, 'height', 300);
        n.appendChild(c, inner);
    ";

fn offset(h: &Harness, id: NodeId) -> Point<Pixels> {
    h.window
        .read_with(&h.cx, |app, _| {
            app.session.dispatcher.scroll_handle(id).unwrap().offset()
        })
        .unwrap()
}

#[gpui::test]
fn scroll_on_a_text_leaf_scrolls_its_container(cx: &mut TestAppContext) {
    let mut h = Harness::load(cx, TEST_ENTRY_PATH, SCROLLER);
    let container = id_where(&mut h, |n| n.children.iter().any(|c| c.text.is_some()));
    let leaf = id_where(&mut h, |n| n.text.is_some());

    h.scroll(leaf, 0.0, -20.0);

    assert_eq!(offset(&h, container).y, px(-20.0));
}

#[gpui::test]
fn scroll_moves_the_horizontal_offset(cx: &mut TestAppContext) {
    let mut h = Harness::load(cx, TEST_ENTRY_PATH, SCROLLER);
    let container = id_where(&mut h, |n| n.children.iter().any(|c| c.text.is_some()));

    h.scroll(container, -25.0, 0.0);

    assert_eq!(offset(&h, container).x, px(-25.0));
}

#[gpui::test]
fn click_aims_at_content_scrolled_out_of_its_viewport(cx: &mut TestAppContext) {
    let mut h = Harness::load(
        cx,
        TEST_ENTRY_PATH,
        r"
            const n = __inca_native__;
            const c = n.createNode('div');
            n.setStyle(c, 'width', 100);
            n.setStyle(c, 'height', 50);
            n.setStyle(c, 'overflow_y', 'scroll');
            n.appendChild(n.rootNodeId(), c);
            const spacer = n.createNode('div');
            n.setStyle(spacer, 'width', 100);
            n.setStyle(spacer, 'height', 200);
            n.appendChild(c, spacer);
            const target = n.createNode('div');
            n.setStyle(target, 'width', 100);
            n.setStyle(target, 'height', 40);
            n.appendChild(c, target);
            ",
    );
    let container = id_where(&mut h, |n| {
        n.children
            .iter()
            .any(|c| c.bounds.is_some_and(|b| b.height == 40.0))
    });
    let target = id_where(&mut h, |n| n.bounds.is_some_and(|b| b.height == 40.0));
    h.scroll(container, 0.0, -100.0);

    let at = h.center_of(target);

    let view = h
        .snapshot()
        .find(&|n| n.id == container)
        .unwrap()
        .bounds
        .unwrap();
    assert!(f32::from(at.y) > view.y + view.height);
}

#[gpui::test]
fn click_on_a_node_that_was_not_drawn_uses_its_ancestor(cx: &mut TestAppContext) {
    let mut h = Harness::load(
        cx,
        TEST_ENTRY_PATH,
        r"
            const n = __inca_native__;
            const hidden = n.createNode('div');
            n.setStyle(hidden, 'width', 60);
            n.setStyle(hidden, 'height', 30);
            n.setStyle(hidden, 'display', 'none');
            n.appendChild(n.rootNodeId(), hidden);
            const inner = n.createNode('div');
            n.setStyle(inner, 'width', 10);
            n.setStyle(inner, 'height', 10);
            n.appendChild(hidden, inner);
            ",
    );
    let snap = h.snapshot();
    let inner = snap.children[0].children[0].clone();
    assert_eq!(inner.bounds, None);
    let ancestor = snap.children[0].bounds.unwrap();

    let at = h.center_of(inner.id);

    assert_eq!(
        at,
        point(
            px(ancestor.x + ancestor.width / 2.0),
            px(ancestor.y + ancestor.height / 2.0)
        )
    );
}

#[gpui::test]
#[should_panic(expected = "InvalidKeystrokeError")]
fn keystrokes_panic_on_an_unparseable_key(cx: &mut TestAppContext) {
    Harness::load(cx, TEST_ENTRY_PATH, "").keystrokes("a-b");
}

#[gpui::test]
#[should_panic(expected = "InvalidKeystrokeError")]
fn key_up_panics_on_an_unparseable_key(cx: &mut TestAppContext) {
    Harness::load(cx, TEST_ENTRY_PATH, "").key_up("a-b");
}

/// A focused 100x50 `box` whose `keydown` and `keyup` listeners log `type:key:metaKey`.
const KEYED: &str = r"
        const n = __inca_native__;
        const box = n.createNode('div');
        n.setStyle(box, 'width', 100);
        n.setStyle(box, 'height', 50);
        n.setAttribute(box, 'tabindex', '0');
        n.appendChild(n.rootNodeId(), box);
        globalThis.log = [];
        globalThis.cb = {
            0: (e) => {
                globalThis.log.push(`${e.type}:${e.key}:${e.metaKey}`);
            },
        };
        listen(box, 'keydown', 0);
        listen(box, 'keyup', 0);
        n.focusNode(box);
    ";

#[gpui::test]
fn keystrokes_press_each_key_in_order_and_release_none(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, KEYED);

    h.keystrokes("a cmd-b x");

    assert_eq!(log(&h), "keydown:a:false,keydown:b:true,keydown:x:false");
}

#[gpui::test]
fn key_up_releases_each_key_in_order(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, KEYED);

    h.key_up("a cmd-b x");

    assert_eq!(log(&h), "keyup:a:false,keyup:b:true,keyup:x:false");
}

#[gpui::test]
#[should_panic(expected = "the window is open")]
fn settle_panics_once_the_window_has_closed(cx: &mut TestAppContext) {
    let mut h = Harness::load(cx, TEST_ENTRY_PATH, "");
    h.cx.update(|window, _| window.remove_window());
    h.settle();
}
