// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

/// `a` holds `b`, which holds `c` and `d`. Every hover event is recorded as
/// `type@currentTarget/target/relatedTarget` in `globalThis.seen`, with `x` for a missing
/// related target.
fn hover_tree() -> (EventDispatcher, [NodeId; 4]) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [a, b, c, d] = {
        let mut h = host.borrow_mut();
        let ids = [(); 4].map(|()| h.tree.create_node("div").unwrap());
        h.tree.append_child(ids[0], ids[1]).unwrap();
        h.tree.append_child(ids[1], ids[2]).unwrap();
        h.tree.append_child(ids[1], ids[3]).unwrap();
        ids
    };
    dispatcher
        .engine
        .eval::<()>("globalThis.seen = [];")
        .unwrap();
    for node in [a, b, c, d] {
        for event in ["mouseover", "mouseout", "mouseenter", "mouseleave"] {
            listen(
                &dispatcher,
                node,
                event,
                "(e) => seen.push(`${e.type}@${e.currentTarget}/${e.target}/${e.relatedTarget ?? 'x'}`)",
            );
        }
    }
    (dispatcher, [a, b, c, d])
}

fn hover(
    cx: &mut TestAppContext,
    dispatcher: &EventDispatcher,
    next: Option<NodeId>,
) -> Vec<String> {
    let cx = cx.add_empty_window();
    cx.update(|window, cx| dispatcher.report_hover(next, window, cx));
    serde_json::from_value(seen(dispatcher)).unwrap()
}

#[gpui::test]
fn entering_a_nested_node_fires_over_then_enter_on_each_ancestor(cx: &mut TestAppContext) {
    let (dispatcher, [a, b, c, _]) = hover_tree();

    let got = hover(cx, &dispatcher, Some(c));

    assert_eq!(
        got,
        [
            format!("mouseover@{c}/{c}/x"),
            format!("mouseover@{b}/{c}/x"),
            format!("mouseover@{a}/{c}/x"),
            format!("mouseenter@{a}/{a}/x"),
            format!("mouseenter@{b}/{b}/x"),
            format!("mouseenter@{c}/{c}/x"),
        ]
    );
}

#[gpui::test]
fn moving_to_a_sibling_fires_out_leave_over_enter_with_related_targets(cx: &mut TestAppContext) {
    let (dispatcher, [a, b, c, d]) = hover_tree();
    hover(cx, &dispatcher, Some(c));

    let got = hover(cx, &dispatcher, Some(d));

    assert_eq!(
        got,
        [
            format!("mouseout@{c}/{c}/{d}"),
            format!("mouseout@{b}/{c}/{d}"),
            format!("mouseout@{a}/{c}/{d}"),
            format!("mouseleave@{c}/{c}/{d}"),
            format!("mouseover@{d}/{d}/{c}"),
            format!("mouseover@{b}/{d}/{c}"),
            format!("mouseover@{a}/{d}/{c}"),
            format!("mouseenter@{d}/{d}/{c}"),
        ]
    );
}

#[gpui::test]
fn leaving_everything_fires_out_then_leave_from_the_inside_out(cx: &mut TestAppContext) {
    let (dispatcher, [a, b, _, d]) = hover_tree();
    hover(cx, &dispatcher, Some(d));

    let got = hover(cx, &dispatcher, None);

    assert_eq!(
        got,
        [
            format!("mouseout@{d}/{d}/x"),
            format!("mouseout@{b}/{d}/x"),
            format!("mouseout@{a}/{d}/x"),
            format!("mouseleave@{d}/{d}/x"),
            format!("mouseleave@{b}/{b}/x"),
            format!("mouseleave@{a}/{a}/x"),
        ]
    );
}

#[gpui::test]
fn reporting_the_same_hover_again_fires_nothing(cx: &mut TestAppContext) {
    let (dispatcher, [_, _, c, _]) = hover_tree();
    hover(cx, &dispatcher, Some(c));

    let got = hover(cx, &dispatcher, Some(c));

    assert_eq!(got, Vec::<String>::new());
}
