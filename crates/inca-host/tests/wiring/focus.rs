// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What JS sees of focus. A focus request the window applies on a frame reaches the JS listeners
//! as `focus`, `blur`, `focusin` and `focusout`, a blur that no request starts (an ancestor turning
//! `display: none`) reaches them too, and a node the script destroys drops its focus handle. A
//! focused node that a removal detaches or an insert moves loses focus at the call, with no `blur`
//! or `focusout`, and never regains it.

#![allow(clippy::unwrap_used)]

use crate::support::rig::{self as wiring, Rig};
use gpui::{TestAppContext, prelude::*};
use inca_jsenv::Engine;
use inca_node::NodeId;
use inca_test_macros::gpui_cases;

/// Two focusable sibling nodes wrapped in a common parent, the root
/// `TestRoot` renders.
fn build_focusable_pair() -> (Rig, NodeId, NodeId) {
    let (host, parent, children) = wiring::focusable_tree(2, false);
    (host, parent, children[0])
}

/// Records `type:target` of `"focus"` and `"blur"` on the children of
/// `parent`.
fn install_focus_recorder(engine: &Engine, host: &Rig, parent: NodeId) {
    engine.eval::<()>("globalThis.seen = [];").unwrap();
    let children = host
        .document
        .borrow()
        .tree
        .get(parent)
        .unwrap()
        .children()
        .to_vec();
    wiring::listen_each(
        engine,
        &children,
        &["focus", "blur"],
        "(event) => { globalThis.seen.push(`${event.type}:${event.target}`); }",
        false,
    );
}

#[gpui::test]
fn focus_node_dispatches_focus_next_frame(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let engine = wiring::engine_for(&host);
    install_focus_recorder(&engine, &host, parent);
    let window = wiring::mount_active(cx, &host, &engine, parent);
    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        "[]",
        "mounting must never touch the JS engine"
    );

    cx.update_window(window.into(), |_root, window, cx| {
        host.document.borrow_mut().request_focus(a, true);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        format!(r#"["focus:{a}"]"#)
    );
}

/// `blurNode` when nothing is focused must not fire a spurious `"blur"`.
#[gpui::test]
fn blur_node_with_nothing_focused_reports_nothing(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let engine = wiring::engine_for(&host);
    install_focus_recorder(&engine, &host, parent);
    let window = wiring::mount_active(cx, &host, &engine, parent);

    cx.update_window(window.into(), |_root, window, cx| {
        host.document.borrow_mut().request_blur(a);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        "[]"
    );
}

/// Destroying a focused node reports no `"blur"`. Its listeners are
/// already gone by the time `destroyNode` returns, the same as any other
/// event kind's registrations on a destroyed node.
#[gpui::test]
fn destroying_a_focused_node_reports_no_blur(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let engine = wiring::engine_for(&host);
    install_focus_recorder(&engine, &host, parent);
    let window = wiring::mount_active(cx, &host, &engine, parent);

    cx.update_window(window.into(), |_root, window, cx| {
        host.document.borrow_mut().request_focus(a, true);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
    engine.eval::<()>("globalThis.seen = [];").unwrap();

    cx.update_window(window.into(), |_root, window, cx| {
        host.document.borrow_mut().tree.destroy_node(a);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        "[]"
    );
}

/// `focusNode` queues its requests: two calls before the next frame both
/// take effect, in order.
#[gpui::test]
fn two_focus_node_calls_before_the_next_frame_both_take_effect(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let b = {
        let host = host.document.borrow();
        host.tree.get(parent).unwrap().children()[1]
    };
    let engine = wiring::engine_for(&host);
    install_focus_recorder(&engine, &host, parent);
    let window = wiring::mount_active(cx, &host, &engine, parent);

    cx.update_window(window.into(), |_root, window, cx| {
        host.document.borrow_mut().request_focus(a, true);
        host.document.borrow_mut().request_focus(b, true);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        format!(r#"["focus:{a}","blur:{a}","focus:{b}"]"#),
        "both requests must take effect, in order"
    );
}

/// Runs `script` against the host's bindings, then draws one frame.
fn run_then_draw(
    cx: &mut TestAppContext,
    engine: &Engine,
    host: &Rig,
    parent: NodeId,
    script: &str,
) {
    let window = wiring::mount_root(cx, host, &wiring::engine_for(host), parent);
    engine.eval::<()>(script).unwrap();
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.run_until_parked();
}

#[gpui::test]
fn focus_then_destroy_in_one_tick_leaves_no_handle(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let engine = wiring::engine_for(&host);

    let script = format!("__inca_native__.focusNode({a}); __inca_native__.destroyNode({a});");
    run_then_draw(cx, &engine, &host, parent, &script);

    assert!(host.cells.focus_handle(a).is_none());
}

/// `blurNode(id)` blurs only when `id` is the focused node: another node's
/// id, or an unknown one, leaves focus alone.
#[gpui_cases]
#[gpui::test]
#[case::another_node(Some(1))]
#[case::unknown_node(None)]
fn blur_node_of_a_node_that_is_not_focused_is_a_no_op(
    cx: &mut TestAppContext,
    other: Option<usize>,
) {
    let (host, parent, a) = build_focusable_pair();
    let other_id = other.map_or(999, |i| {
        host.document.borrow().tree.get(parent).unwrap().children()[i]
    });
    let engine = wiring::engine_for(&host);
    install_focus_recorder(&engine, &host, parent);
    let window = wiring::mount_active(cx, &host, &engine, parent);
    cx.update_window(window.into(), |_root, window, cx| {
        host.document.borrow_mut().request_focus(a, true);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
    engine.eval::<()>("globalThis.seen = [];").unwrap();

    cx.update_window(window.into(), |_root, window, cx| {
        host.document.borrow_mut().request_blur(other_id);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();

    let seen = engine.eval::<String>("JSON.stringify(seen)").unwrap();
    assert_eq!(seen, "[]", "blur of {other_id} must not blur {a}");
    let still_focused = cx
        .update_window(window.into(), |_, window, cx| {
            host.cells.focus_handle(a).unwrap().is_focused(window) && window.focused(cx).is_some()
        })
        .unwrap();
    assert!(still_focused);
}

#[gpui::test]
fn each_focus_transition_gets_its_own_event_id(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let b = host.document.borrow().tree.get(parent).unwrap().children()[1];
    let engine = wiring::engine_for(&host);
    engine.eval::<()>("globalThis.seen = [];").unwrap();
    for node in [a, b] {
        for event in ["focus", "blur"] {
            engine
                .eval::<()>(&format!(
                    r"
                    __inca_test_listen__({node}, '{event}', (event) => {{
                        globalThis.seen.push([event.type, event.eventId]);
                    }});
                    "
                ))
                .unwrap();
        }
    }
    let window = wiring::mount_active(cx, &host, &engine, parent);

    for targets in [vec![a, b], vec![a]] {
        cx.update_window(window.into(), |_root, window, cx| {
            for target in targets {
                host.document.borrow_mut().request_focus(target, true);
            }
            window.draw(cx).clear(cx);
        })
        .unwrap();
        cx.run_until_parked();
    }

    let seen: Vec<(String, u64)> = serde_json::from_str(
        &engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
    )
    .unwrap();
    let names: Vec<&str> = seen.iter().map(|entry| entry.0.as_str()).collect();
    assert_eq!(names, ["focus", "blur", "focus", "blur", "focus"]);
    let ids: Vec<u64> = seen.iter().map(|entry| entry.1).collect();
    assert!(
        ids[0] >= 1 && ids.windows(2).all(|pair| pair[1] > pair[0]),
        "{ids:?}"
    );
}

/// Each focus change fires `blur`, `focusout`, `focus`, `focusin` in that
/// order. `relatedTarget` is the node on the other side of the change.
/// `focusin` and `focusout` reach the parent at phase 3, `focus` and `blur`
/// stay on the node.
#[gpui::test]
fn focus_changes_fire_four_events_with_related_targets(cx: &mut TestAppContext) {
    enum Step {
        Focus(usize),
        Blur(usize),
        RemoveTabindex(usize),
    }
    let (host, parent, a) = build_focusable_pair();
    let b = host.document.borrow().tree.get(parent).unwrap().children()[1];
    let engine = wiring::engine_for(&host);
    engine.eval::<()>("globalThis.seen = [];").unwrap();
    for (node, events) in [
        (a, &["focus", "blur", "focusin", "focusout"][..]),
        (b, &["focus", "blur", "focusin", "focusout"][..]),
        (parent, &["focus", "blur", "focusin", "focusout"][..]),
    ] {
        for event in events {
            engine
                .eval::<()>(&format!(
                    r"
                    __inca_test_listen__({node}, '{event}', (e) => {{
                        const {{ type, target, relatedTarget }} = e;
                        const {{ eventPhase, bubbles }} = e;
                        globalThis.seen.push(
                            `${{type}}:${{target}}:${{relatedTarget}}:${{eventPhase}}:${{bubbles}}`
                        );
                    }});
                    "
                ))
                .unwrap();
        }
    }
    let window = wiring::mount_active(cx, &host, &engine, parent);

    let nodes = [a, b];
    let steps = [
        (
            Step::Focus(0),
            format!("focus:{a}:null:2:false focusin:{a}:null:2:true focusin:{a}:null:3:true"),
        ),
        (
            Step::Focus(1),
            format!(
                concat!(
                    "blur:{a}:{b}:2:false focusout:{a}:{b}:2:true focusout:{a}:{b}:3:true ",
                    "focus:{b}:{a}:2:false focusin:{b}:{a}:2:true focusin:{b}:{a}:3:true"
                ),
                a = a,
                b = b
            ),
        ),
        (
            Step::Blur(1),
            format!("blur:{b}:null:2:false focusout:{b}:null:2:true focusout:{b}:null:3:true"),
        ),
        (
            Step::Focus(1),
            format!("focus:{b}:null:2:false focusin:{b}:null:2:true focusin:{b}:null:3:true"),
        ),
        (
            Step::RemoveTabindex(1),
            format!("blur:{b}:null:2:false focusout:{b}:null:2:true focusout:{b}:null:3:true"),
        ),
    ];
    let mut mismatches = Vec::new();
    for (step, want) in steps {
        engine.eval::<()>("globalThis.seen = [];").unwrap();
        cx.update_window(window.into(), |_root, window, cx| {
            let mut host = host.document.borrow_mut();
            match step {
                Step::Focus(i) => host.request_focus(nodes[i], true),
                Step::Blur(i) => host.request_blur(nodes[i]),
                Step::RemoveTabindex(i) => {
                    host.tree.remove_attribute(nodes[i], "tabindex").unwrap();
                    host.mark_tab_dirty(nodes[i]);
                }
            }
            drop(host);
            window.draw(cx).clear(cx);
        })
        .unwrap();
        cx.run_until_parked();
        let seen = engine.eval::<String>("seen.join(' ')").unwrap();
        if seen != want {
            mismatches.push(format!("want {want}\n got {seen}"));
        }
    }
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

/// `setStyle` on an ancestor re-checks the focused node.
#[gpui::test]
fn an_ancestor_becoming_display_none_blurs_the_focused_node(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let engine = wiring::engine_for(&host);
    engine.eval::<()>("globalThis.seen = [];").unwrap();
    for event in ["blur", "focusout"] {
        engine
            .eval::<()>(&format!(
                r"
                __inca_test_listen__({a}, '{event}', (e) => {{
                    globalThis.seen.push(`${{e.type}}:${{e.relatedTarget}}`);
                }});
                "
            ))
            .unwrap();
    }
    let window = wiring::mount_active(cx, &host, &engine, parent);
    cx.update_window(window.into(), |_, window, cx| {
        host.document.borrow_mut().request_focus(a, true);
        window.draw(cx).clear(cx);
    })
    .unwrap();

    engine
        .eval::<()>(&format!(
            "seen = []; __inca_native__.setStyle({parent}, 'display', 'none');"
        ))
        .unwrap();
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.run_until_parked();

    assert_eq!(
        engine.eval::<String>("seen.join(' ')").unwrap(),
        "blur:null focusout:null"
    );
    assert!(host.cells.focus_handle(a).is_none());
}

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
    let (host, parent, children) = wiring::focusable_tree(2, false);
    let (leaf, sibling) = (children[0], children[1]);
    let root = host.document.borrow().root();
    let mut top = leaf;
    if nested {
        let mut host_mut = host.document.borrow_mut();
        let mid = host_mut.tree.create_node("div").unwrap();
        host_mut.tree.remove_child(parent, leaf).unwrap();
        host_mut.tree.append_child(mid, leaf).unwrap();
        host_mut.tree.append_child(parent, mid).unwrap();
        top = mid;
    }
    let engine = wiring::engine_for(&host);
    engine.eval::<()>("globalThis.seen = [];").unwrap();
    wiring::listen_each(
        &engine,
        &[leaf],
        &["blur", "focusout", "focus", "focusin"],
        "(e) => { globalThis.seen.push(`${e.type}:${e.target}`); }",
        false,
    );
    let window = wiring::mount_active(cx, &host, &engine, parent);
    cx.update_window(window.into(), |_root, window, cx| {
        host.document.borrow_mut().request_focus(leaf, true);
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
            host.cells.focused_node(window, cx)
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

#[gpui_cases]
#[gpui::test]
#[case::the_focused_node(false)]
#[case::an_ancestor(true)]
fn replacing_the_focused_node_or_an_ancestor_drops_focus_at_the_call(
    cx: &mut TestAppContext,
    #[case] nested: bool,
) {
    let out = focus_then_run(cx, nested, |ids| {
        format!(
            r"
            const n = __inca_native__;
            n.replaceChild({}, n.createNode('div'), {});
            ",
            ids.parent, ids.top
        )
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

#[gpui_cases]
#[gpui::test]
#[case::append("insertNodes(P, 'append', [T])")]
#[case::replace_children("insertNodes(P, 'replaceChildren', [])")]
#[case::replace_with_no_node("insertNodes(T, 'replaceWith', [])")]
fn inserting_nodes_over_the_focused_node_drops_focus_at_the_call(
    cx: &mut TestAppContext,
    #[case] call: &str,
) {
    let out = focus_then_run(cx, false, |ids| {
        let call = call
            .replace('P', &ids.parent.to_string())
            .replace('T', &ids.top.to_string());
        format!("__inca_native__.{call}")
    });

    assert_dropped(&out);
}

#[gpui_cases]
#[gpui::test]
#[case::append("insertNodes(P, 'append', [S])")]
#[case::replace_with_no_node("insertNodes(S, 'replaceWith', [])")]
fn inserting_nodes_over_an_unrelated_node_keeps_focus(cx: &mut TestAppContext, #[case] call: &str) {
    let out = focus_then_run(cx, false, |ids| {
        let call = call
            .replace('P', &ids.parent.to_string())
            .replace('S', &ids.sibling.to_string());
        format!("__inca_native__.{call}")
    });

    assert_eq!(out.at_call, Some(out.leaf));
    assert_eq!(out.after_frame, Some(out.leaf));
    assert_eq!(out.seen, "[]");
}

#[gpui::test]
fn removing_from_a_parent_that_is_not_the_parent_keeps_focus(cx: &mut TestAppContext) {
    let out = focus_then_run(cx, false, |ids| {
        format!(
            r"
            try {{
                {};
            }} catch (e) {{
            }}
            ",
            native("removeChild", &[ids.root, ids.top])
        )
    });

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
    let (host, parent, _) = wiring::focusable_tree(0, false);
    let engine = wiring::engine_for(&host);
    let window = wiring::mount_active(cx, &host, &engine, parent);
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
    let (host, parent, _) = wiring::focusable_tree(0, false);
    let engine = wiring::engine_for(&host);
    let window = wiring::mount_active(cx, &host, &engine, parent);
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
