// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A focused node that `removeChild` detaches or an insert moves loses focus
//! at the call, with no `blur` or `focusout`, and never regains it.

#![allow(clippy::unwrap_used)]

use gpui::{TestAppContext, prelude::*};
use inca_bridge::test_support;
use inca_gpui::NodeId;
use inca_test_macros::gpui_cases;

struct Ids {
    root: NodeId,
    parent: NodeId,
    /// The focused leaf, or the node holding it when nested.
    top: NodeId,
    sibling: NodeId,
}

struct Outcome {
    /// `focusedNode()` read right after the script, before any frame.
    at_call: Option<NodeId>,
    /// The window's focused node after one drawn frame.
    after_frame: Option<NodeId>,
    /// Focus events the leaf received after the script.
    seen: String,
    leaf: NodeId,
}

/// Focuses the leaf, runs the script `build` returns, then draws one frame.
fn focus_then_run(
    cx: &mut TestAppContext,
    nested: bool,
    build: impl Fn(&Ids) -> String,
) -> Outcome {
    let (host, parent, children) = test_support::focusable_tree(2, false);
    let (leaf, sibling) = (children[0], children[1]);
    let root = host.borrow().root;
    let mut top = leaf;
    if nested {
        let mut host_mut = host.borrow_mut();
        let mid = host_mut.tree.create_node("div").unwrap();
        host_mut.tree.remove_child(parent, leaf).unwrap();
        host_mut.tree.append_child(mid, leaf).unwrap();
        host_mut.tree.append_child(parent, mid).unwrap();
        top = mid;
    }
    let engine = test_support::engine_for(&host);
    engine.eval::<()>("globalThis.seen = [];").unwrap();
    test_support::listen_each(
        &engine,
        &[leaf],
        &["blur", "focusout", "focus", "focusin"],
        "(e) => { globalThis.seen.push(`${e.type}:${e.target}`); }",
        false,
    );
    let window = test_support::mount_active(cx, &host, &engine, parent);
    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().focus.request_focus(leaf, true);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
    engine.eval::<()>("globalThis.seen = [];").unwrap();

    let ids = Ids {
        root,
        parent,
        top,
        sibling,
    };
    let script = format!("{}; __inca_native__.focusedNode();", build(&ids));
    let at_call = engine.eval::<Option<NodeId>>(&script).unwrap();
    let after_frame = cx
        .update_window(window.into(), |_root, window, cx| {
            window.draw(cx).clear(cx);
            host.borrow().focus.focused_node(window, cx)
        })
        .unwrap();
    cx.run_until_parked();
    let seen = engine
        .eval::<String>("JSON.stringify(globalThis.seen)")
        .unwrap();
    Outcome {
        at_call,
        after_frame,
        seen,
        leaf,
    }
}

fn native(call: &str, args: &[NodeId]) -> String {
    let args: Vec<String> = args.iter().map(ToString::to_string).collect();
    format!("__inca_native__.{call}({})", args.join(", "))
}

fn assert_dropped(out: &Outcome) {
    assert_eq!(out.at_call, None);
    assert_eq!(out.after_frame, None);
    assert_eq!(out.seen, "[]");
}

#[gpui_cases]
#[gpui::test]
#[case::the_focused_node(false)]
#[case::an_ancestor(true)]
fn removing_the_focused_node_or_an_ancestor_drops_focus_at_the_call(
    cx: &mut TestAppContext,
    #[case] nested: bool,
) {
    let out = focus_then_run(cx, nested, |ids| {
        native("removeChild", &[ids.parent, ids.top])
    });

    assert_dropped(&out);
}

#[gpui::test]
fn removing_an_unrelated_node_keeps_focus(cx: &mut TestAppContext) {
    let out = focus_then_run(cx, false, |ids| {
        native("removeChild", &[ids.parent, ids.sibling])
    });

    assert_eq!(out.at_call, Some(out.leaf));
    assert_eq!(out.after_frame, Some(out.leaf));
    assert_eq!(out.seen, "[]");
}

#[gpui::test]
fn re_appending_the_removed_node_in_the_same_call_leaves_it_unfocused(cx: &mut TestAppContext) {
    let out = focus_then_run(cx, false, |ids| {
        format!(
            "{}; {}",
            native("removeChild", &[ids.parent, ids.top]),
            native("appendChild", &[ids.parent, ids.top])
        )
    });

    assert_dropped(&out);
}

#[gpui_cases]
#[gpui::test]
#[case::append_child(false)]
#[case::insert_before(true)]
fn moving_the_focused_node_drops_focus(cx: &mut TestAppContext, #[case] anchored: bool) {
    let out = focus_then_run(cx, false, |ids| {
        if anchored {
            native("insertBefore", &[ids.parent, ids.top, ids.sibling])
        } else {
            native("appendChild", &[ids.parent, ids.top])
        }
    });

    assert_dropped(&out);
}

#[gpui_cases]
#[gpui::test]
#[case::append_child(false)]
#[case::insert_before(true)]
fn moving_an_unrelated_node_keeps_focus(cx: &mut TestAppContext, #[case] anchored: bool) {
    let out = focus_then_run(cx, false, |ids| {
        if anchored {
            native("insertBefore", &[ids.parent, ids.sibling, ids.top])
        } else {
            native("appendChild", &[ids.parent, ids.sibling])
        }
    });

    assert_eq!(out.at_call, Some(out.leaf));
    assert_eq!(out.after_frame, Some(out.leaf));
    assert_eq!(out.seen, "[]");
}

#[gpui::test]
fn removing_from_a_parent_that_is_not_the_parent_keeps_focus(cx: &mut TestAppContext) {
    let out = focus_then_run(cx, false, |ids| native("removeChild", &[ids.root, ids.top]));

    assert_eq!(out.at_call, Some(out.leaf));
    assert_eq!(out.after_frame, Some(out.leaf));
    assert_eq!(out.seen, "[]");
}

#[gpui_cases]
#[gpui::test]
#[case::insert_before_itself("insertBefore(P, T, T)", false)]
#[case::append_when_last("appendChild(P, T)", true)]
#[case::insert_before_null_when_last("insertBefore(P, T, null)", true)]
fn a_move_that_leaves_the_order_alone_still_drops_focus(
    cx: &mut TestAppContext,
    #[case] call: &str,
    #[case] make_last: bool,
) {
    let out = focus_then_run(cx, false, |ids| {
        let mut script = String::new();
        if make_last {
            script += &native("insertBefore", &[ids.parent, ids.sibling, ids.top]);
            script += "; ";
        }
        let call = call
            .replace('P', &ids.parent.to_string())
            .replace('T', &ids.top.to_string());
        script + "__inca_native__." + &call
    });

    assert_dropped(&out);
}

#[gpui::test]
fn a_node_that_was_never_attached_cannot_take_focus(cx: &mut TestAppContext) {
    let (host, parent, _) = test_support::focusable_tree(0, false);
    let engine = test_support::engine_for(&host);
    let window = test_support::mount_active(cx, &host, &engine, parent);
    engine
        .eval::<()>(
            "const n = __inca_native__; globalThis.x = n.createNode('div');\
             n.setAttribute(x, 'tabindex', '0'); n.focusNode(x);",
        )
        .unwrap();
    cx.update_window(window.into(), |_root, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<Option<NodeId>>("__inca_native__.focusedNode()")
            .unwrap(),
        None
    );
}

#[gpui::test]
fn focusing_a_node_after_removing_it_in_the_same_call_leaves_no_focus(cx: &mut TestAppContext) {
    let out = focus_then_run(cx, false, |ids| {
        format!(
            "{}; {}",
            native("removeChild", &[ids.parent, ids.top]),
            native("focusNode", &[ids.top])
        )
    });

    assert_dropped(&out);
}

#[gpui_cases]
#[gpui::test]
#[case::moved("appendChild(P, T)")]
#[case::removed_and_appended("removeChild(P, T); __inca_native__.appendChild(P, T)")]
fn focusing_a_node_in_the_call_that_moved_it_focuses_it_without_a_blur(
    cx: &mut TestAppContext,
    #[case] change: &str,
) {
    let out = focus_then_run(cx, false, |ids| {
        let change = change
            .replace('P', &ids.parent.to_string())
            .replace('T', &ids.top.to_string());
        format!(
            "__inca_native__.{change}; {}",
            native("focusNode", &[ids.top])
        )
    });

    assert_eq!(out.at_call, Some(out.leaf));
    assert_eq!(out.after_frame, Some(out.leaf));
    assert_eq!(
        out.seen,
        format!(r#"["focus:{0}","focusin:{0}"]"#, out.leaf)
    );
}

#[gpui::test]
fn a_node_focused_before_its_first_insert_takes_focus_when_inserted(cx: &mut TestAppContext) {
    let (host, parent, _) = test_support::focusable_tree(0, false);
    let engine = test_support::engine_for(&host);
    let window = test_support::mount_active(cx, &host, &engine, parent);
    let node = engine
        .eval::<NodeId>(
            "const n = __inca_native__; const x = n.createNode('div');\
             n.setAttribute(x, 'tabindex', '0'); n.focusNode(x);\
             n.appendChild(n.rootNodeId(), x); x;",
        )
        .unwrap();
    cx.update_window(window.into(), |_root, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<Option<NodeId>>("__inca_native__.focusedNode()")
            .unwrap(),
        Some(node)
    );
}
