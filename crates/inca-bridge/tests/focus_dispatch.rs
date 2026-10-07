// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration tests for `inca_bridge::focus`: `focusNode`/`blurNode`
//! (`crate::bindings`) through to the `"focus"`/`"blur"` dispatched into
//! JS, via a real `FocusRegistry::apply_pending` running once per frame —
//! see `FocusableRoot::render` below.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Context, Render, TestAppContext, Window, prelude::*};
use inca_bridge::{EventDispatcher, Host};
use inca_gpui::{NodeId, render_tree_with_events};
use inca_jsenv::Engine;

struct FocusableRoot {
    host: Rc<RefCell<Host>>,
    node: NodeId,
    dispatcher: EventDispatcher,
}

impl Render for FocusableRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let transitions = {
            let host = &mut *self.host.borrow_mut();
            host.focus.apply_pending(&host.tree, window, cx)
        };
        for transition in &transitions {
            transition.dispatch(&self.dispatcher, window, cx);
        }
        let host = self.host.borrow();
        render_tree_with_events(&host.tree, self.node, &self.dispatcher).unwrap()
    }
}

/// Two sibling nodes, both listening for `"focus"`/`"blur"` via the same
/// callback id, wrapped in a common parent — the root `FocusableRoot`
/// renders.
fn build_focusable_pair() -> (Rc<RefCell<Host>>, NodeId, NodeId) {
    let host = Rc::new(RefCell::new(Host::default()));
    let mut host_mut = host.borrow_mut();
    let parent = host_mut.tree.create_node("div").unwrap();
    let a = host_mut.tree.create_node("div").unwrap();
    let b = host_mut.tree.create_node("div").unwrap();
    host_mut.tree.append_child(parent, a).unwrap();
    host_mut.tree.append_child(parent, b).unwrap();
    for node in [a, b] {
        host_mut.listeners.register(node, "focus", 0);
        host_mut.listeners.register(node, "blur", 0);
        host_mut.tree.set_attribute(node, "tabindex", "0").unwrap();
        host_mut.focus.mark_tab_dirty(node);
    }
    drop(host_mut);
    (host, parent, a)
}

fn install_focus_recorder(engine: &Engine) {
    engine
        .eval::<()>(
            "globalThis.seen = []; \
             globalThis.__inca_callbacks__ = { \
                0: (event) => { globalThis.seen.push(`${event.type}:${event.target}`); } \
             };",
        )
        .unwrap();
}

#[gpui::test]
fn focus_node_dispatches_focus_next_frame(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let engine = Rc::new(Engine::new().unwrap());
    install_focus_recorder(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| FocusableRoot {
        host: Rc::clone(&host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();
    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        "[]",
        "mounting must never touch the JS engine"
    );

    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().focus.request_focus(a);
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

#[gpui::test]
fn moving_focus_to_another_node_blurs_the_first(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let b = {
        let host = host.borrow();
        host.tree.get(parent).unwrap().children()[1]
    };
    let engine = Rc::new(Engine::new().unwrap());
    install_focus_recorder(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| FocusableRoot {
        host: Rc::clone(&host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();

    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().focus.request_focus(a);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
    engine.eval::<()>("globalThis.seen = [];").unwrap();

    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().focus.request_focus(b);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        format!(r#"["blur:{a}","focus:{b}"]"#)
    );
}

#[gpui::test]
fn blur_node_blurs_whatever_is_focused(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let engine = Rc::new(Engine::new().unwrap());
    install_focus_recorder(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| FocusableRoot {
        host: Rc::clone(&host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();

    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().focus.request_focus(a);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
    engine.eval::<()>("globalThis.seen = [];").unwrap();

    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().focus.request_blur(a);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        format!(r#"["blur:{a}"]"#)
    );
}

/// `blurNode` when nothing is focused must not fire a spurious `"blur"`.
#[gpui::test]
fn blur_node_with_nothing_focused_reports_nothing(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let engine = Rc::new(Engine::new().unwrap());
    install_focus_recorder(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| FocusableRoot {
        host: Rc::clone(&host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();

    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().focus.request_blur(a);
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

/// Destroying a focused node reports no `"blur"` — its listeners are
/// already gone by the time `destroyNode` returns, the same as any other
/// event kind's registrations on a destroyed node.
#[gpui::test]
fn destroying_a_focused_node_reports_no_blur(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let engine = Rc::new(Engine::new().unwrap());
    install_focus_recorder(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| FocusableRoot {
        host: Rc::clone(&host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();

    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().focus.request_focus(a);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
    engine.eval::<()>("globalThis.seen = [];").unwrap();

    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().tree.destroy_node(a);
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

/// `focusNode` queues its requests — two calls before the next frame both
/// take effect, in order, the same as the DOM's synchronous `.focus()`.
#[gpui::test]
fn two_focus_node_calls_before_the_next_frame_both_take_effect(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let b = {
        let host = host.borrow();
        host.tree.get(parent).unwrap().children()[1]
    };
    let engine = Rc::new(Engine::new().unwrap());
    install_focus_recorder(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| FocusableRoot {
        host: Rc::clone(&host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();

    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().focus.request_focus(a);
        host.borrow_mut().focus.request_focus(b);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        format!(r#"["focus:{a}","blur:{a}","focus:{b}"]"#),
        "both requests must take effect, in order, like the DOM's synchronous focus()"
    );
}

/// Runs `script` against the host's bindings, then draws one frame.
fn run_then_draw(
    cx: &mut TestAppContext,
    engine: &Engine,
    host: &Rc<RefCell<Host>>,
    parent: NodeId,
    script: &str,
) {
    engine
        .with(|ctx| inca_bridge::bindings::install(&ctx, host))
        .unwrap();
    let dispatcher = EventDispatcher::new(Rc::new(Engine::new().unwrap()), Rc::clone(host));
    let window = cx.add_window(|_, _| FocusableRoot {
        host: Rc::clone(host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    engine.eval::<()>(script).unwrap();
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.run_until_parked();
}

#[gpui::test]
fn focus_then_destroy_in_one_tick_leaves_no_handle(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let engine = Engine::new().unwrap();

    let script = format!("__inca_native__.focusNode({a}); __inca_native__.destroyNode({a});");
    run_then_draw(cx, &engine, &host, parent, &script);

    assert!(host.borrow().focus.handle(a).is_none());
}

#[gpui::test]
fn focus_on_an_unknown_node_leaves_no_handle(cx: &mut TestAppContext) {
    let (host, parent, _a) = build_focusable_pair();
    let engine = Engine::new().unwrap();

    run_then_draw(
        cx,
        &engine,
        &host,
        parent,
        "__inca_native__.focusNode(999);",
    );

    assert!(host.borrow().focus.handle(999).is_none());
}

/// `blurNode(id)` blurs only when `id` is the focused node: another node's
/// id, or an unknown one, leaves focus alone.
#[gpui::test]
fn blur_node_of_a_node_that_is_not_focused_is_a_no_op(cx: &mut TestAppContext) {
    for other in [Some(1), None] {
        let (host, parent, a) = build_focusable_pair();
        let other_id = other.map_or(999, |i| {
            host.borrow().tree.get(parent).unwrap().children()[i]
        });
        let engine = Rc::new(Engine::new().unwrap());
        install_focus_recorder(&engine);
        let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));
        let window = cx.add_window(|_, _| FocusableRoot {
            host: Rc::clone(&host),
            node: parent,
            dispatcher,
        });
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
        cx.update_window(window.into(), |_, window, _cx| window.activate_window())
            .unwrap();
        cx.update_window(window.into(), |_root, window, cx| {
            host.borrow_mut().focus.request_focus(a);
            window.draw(cx).clear(cx);
        })
        .unwrap();
        cx.run_until_parked();
        engine.eval::<()>("globalThis.seen = [];").unwrap();

        cx.update_window(window.into(), |_root, window, cx| {
            host.borrow_mut().focus.request_blur(other_id);
            window.draw(cx).clear(cx);
        })
        .unwrap();
        cx.run_until_parked();

        let seen = engine.eval::<String>("JSON.stringify(seen)").unwrap();
        assert_eq!(seen, "[]", "blur of {other_id} must not blur {a}");
        let still_focused = cx
            .update_window(window.into(), |_, window, cx| {
                host.borrow().focus.handle(a).unwrap().is_focused(window)
                    && window.focused(cx).is_some()
            })
            .unwrap();
        assert!(still_focused);
    }
}

#[gpui::test]
fn focus_and_blur_each_carry_an_event_id_that_grows(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let b = host.borrow().tree.get(parent).unwrap().children()[1];
    let engine = Rc::new(Engine::new().unwrap());
    engine
        .eval::<()>(
            "globalThis.seen = []; \
             globalThis.__inca_callbacks__ = { \
                0: (event) => { globalThis.seen.push([event.type, event.eventId]); } \
             };",
        )
        .unwrap();
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));
    let window = cx.add_window(|_, _| FocusableRoot {
        host: Rc::clone(&host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();

    for target in [a, b] {
        cx.update_window(window.into(), |_root, window, cx| {
            host.borrow_mut().focus.request_focus(target);
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
    assert_eq!(names, ["focus", "blur", "focus"]);
    assert!(seen[0].1 >= 1);
    assert!(seen[1].1 > seen[0].1);
    assert!(seen[2].1 > seen[1].1);
}

#[gpui::test]
fn focus_requests_in_one_frame_share_a_focus_id_and_a_later_frame_gets_a_new_one(
    cx: &mut TestAppContext,
) {
    let (host, parent, a) = build_focusable_pair();
    let b = host.borrow().tree.get(parent).unwrap().children()[1];
    let engine = Rc::new(Engine::new().unwrap());
    engine
        .eval::<()>(
            "globalThis.seen = []; \
             globalThis.__inca_callbacks__ = { \
                0: (event) => { globalThis.seen.push([event.type, event.eventId]); } \
             };",
        )
        .unwrap();
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));
    let window = cx.add_window(|_, _| FocusableRoot {
        host: Rc::clone(&host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();

    for targets in [vec![a, b], vec![a]] {
        cx.update_window(window.into(), |_root, window, cx| {
            for target in targets {
                host.borrow_mut().focus.request_focus(target);
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
    assert_eq!(seen[0].1, seen[2].1, "same frame, same focus id");
    assert_ne!(seen[0].1, seen[1].1);
    assert!(seen[4].1 > seen[2].1, "a later frame takes a new focus id");
    assert!(seen[3].1 > seen[2].1);
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
    let b = host.borrow().tree.get(parent).unwrap().children()[1];
    for (node, events) in [
        (a, &["focusin", "focusout"][..]),
        (b, &["focusin", "focusout"][..]),
        (parent, &["focus", "blur", "focusin", "focusout"][..]),
    ] {
        for event in events {
            host.borrow_mut().listeners.register(node, *event, 0);
        }
    }
    let engine = Rc::new(Engine::new().unwrap());
    engine
        .eval::<()>(
            "globalThis.seen = []; \
             globalThis.__inca_callbacks__ = { 0: (e) => { globalThis.seen.push( \
             `${e.type}:${e.target}:${e.relatedTarget}:${e.eventPhase}:${e.bubbles}`); } };",
        )
        .unwrap();
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));
    let window = cx.add_window(|_, _| FocusableRoot {
        host: Rc::clone(&host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();

    let nodes = [a, b];
    let steps = [
        (
            Step::Focus(0),
            format!("focus:{a}:null:2:false focusin:{a}:null:2:true focusin:{a}:null:3:true"),
        ),
        (
            Step::Focus(1),
            format!(
                "blur:{a}:{b}:2:false focusout:{a}:{b}:2:true focusout:{a}:{b}:3:true \
                 focus:{b}:{a}:2:false focusin:{b}:{a}:2:true focusin:{b}:{a}:3:true"
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
            let mut host = host.borrow_mut();
            match step {
                Step::Focus(i) => host.focus.request_focus(nodes[i]),
                Step::Blur(i) => host.focus.request_blur(nodes[i]),
                Step::RemoveTabindex(i) => {
                    host.tree.remove_attribute(nodes[i], "tabindex").unwrap();
                    host.focus.mark_tab_dirty(nodes[i]);
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
    for event in ["blur", "focusout"] {
        host.borrow_mut().listeners.register(a, event, 0);
    }
    let engine = Rc::new(Engine::new().unwrap());
    engine
        .eval::<()>(
            "globalThis.seen = []; \
             globalThis.__inca_callbacks__ = { 0: (e) => { globalThis.seen.push( \
             `${e.type}:${e.relatedTarget}`); } };",
        )
        .unwrap();
    engine
        .with(|ctx| inca_bridge::bindings::install(&ctx, &host))
        .unwrap();
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));
    let window = cx.add_window(|_, _| FocusableRoot {
        host: Rc::clone(&host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();
    cx.update_window(window.into(), |_, window, cx| {
        host.borrow_mut().focus.request_focus(a);
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
    assert!(host.borrow().focus.handle(a).is_none());
}

/// Parking the window focus on the root container reports no transition,
/// maps to no node and does not flip per frame.
#[gpui::test]
fn the_parked_focus_fires_no_focus_event(cx: &mut TestAppContext) {
    let (host, _parent, _a) = build_focusable_pair();
    let window = cx.add_window(|_, _| gpui::Empty);
    cx.update_window(window.into(), |_, window, cx| {
        for _ in 0..2 {
            let host = &mut *host.borrow_mut();
            let transitions = host.focus.apply_pending(&host.tree, window, cx);
            assert!(transitions.is_empty());
            assert!(window.focused(cx).is_some());
            assert_eq!(host.focus.focused_node(window, cx), None);
        }
    })
    .unwrap();
}

/// A root with `tabindex` takes focus like any node, and the parked focus
/// leaves it alone.
#[gpui::test]
fn a_root_with_tabindex_still_focuses_normally(cx: &mut TestAppContext) {
    let (host, _parent, _a) = build_focusable_pair();
    let root = host.borrow().root;
    {
        let mut host = host.borrow_mut();
        host.tree.set_attribute(root, "tabindex", "0").unwrap();
        host.focus.mark_tab_dirty(root);
    }
    let window = cx.add_window(|_, _| gpui::Empty);
    cx.update_window(window.into(), |_, window, cx| {
        let host = &mut *host.borrow_mut();
        assert!(host.focus.apply_pending(&host.tree, window, cx).is_empty());
        host.focus.request_focus(root);
        let transitions = host.focus.apply_pending(&host.tree, window, cx);
        assert_eq!(transitions.len(), 1);
        assert_eq!(host.focus.focused_node(window, cx), Some(root));
        assert!(host.focus.apply_pending(&host.tree, window, cx).is_empty());
    })
    .unwrap();
}
