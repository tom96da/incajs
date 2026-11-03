// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::rc::Rc;

use inca_gpui::{EventPayload, NodeId};
use inca_jsenv::{Engine, EngineError};
use rquickjs::{Ctx, Object, Result as JsResult};

use super::construct::create;
use super::{Flags, IncaEvent};
use crate::EventTargetId;
use crate::bindings::Host;
use listeners::deliver;

mod listeners;
mod script;
mod targets;

pub(crate) use script::{click, dispatch_event};
pub(super) use targets::targets_of;

/// What one walk needs to know.
pub(crate) struct Walk<'a> {
    pub(crate) name: &'a str,
    pub(crate) flags: Flags,
    pub(crate) target: EventTargetId,
    /// The nodes whose listeners do not run. A cut silences document and window too.
    pub(crate) cut: &'a [NodeId],
    pub(crate) payload: &'a EventPayload,
    /// The value of `isTrusted` on the event the walk creates.
    pub(crate) trusted: bool,
}

/// What a walk asked of its caller.
#[derive(Default)]
pub(crate) struct Outcome {
    /// A listener called `preventDefault()` on a cancelable event.
    pub(crate) canceled: bool,
    /// The walk entered the engine, so jobs may be queued.
    pub(crate) entered: bool,
}

/// The targets an event fired at `target` travels through, nearest first: the
/// nodes from `target` up to the host root, then document, then window. A node
/// that is not under the root never reaches document or window.
pub(crate) fn path_from(host: &Host, target: EventTargetId) -> Vec<EventTargetId> {
    let mut path = Vec::new();
    let mut connected = true;
    if let EventTargetId::Node(node) = target {
        let ancestors: Vec<_> = host.tree.ancestors(node).collect();
        connected = ancestors.last().copied() == host.tree.root();
        path.extend(ancestors.into_iter().map(EventTargetId::Node));
    }
    if connected {
        if target != EventTargetId::Window {
            path.push(EventTargetId::Document);
        }
        path.push(EventTargetId::Window);
    }
    path
}

/// The targets whose bubble listeners the event reaches.
fn bubble_targets(flags: Flags, path: &[EventTargetId]) -> &[EventTargetId] {
    if flags.bubbles {
        path
    } else {
        &path[..path.len().min(1)]
    }
}

/// Whether `target` stays silent: a cut node, or document and window while any
/// node is cut.
fn is_cut(target: EventTargetId, cut: &[NodeId]) -> bool {
    match target {
        EventTargetId::Node(node) => cut.contains(&node),
        EventTargetId::Document | EventTargetId::Window => !cut.is_empty(),
    }
}

/// Whether a listener of `name` exists that an event with `flags` fired at
/// `target` reaches. Nodes in `cut` stay silent.
pub(crate) fn listened(
    host: &Host,
    name: &str,
    flags: Flags,
    target: EventTargetId,
    cut: &[NodeId],
) -> bool {
    path_from(host, target)
        .into_iter()
        .enumerate()
        .any(|(depth, on_path)| {
            !is_cut(on_path, cut)
                && host
                    .listeners
                    .listeners_for(on_path, name)
                    .iter()
                    .any(|l| l.capture || flags.bubbles || depth == 0)
        })
}

pub(super) fn next_event_id(host: &Rc<RefCell<Host>>) -> u64 {
    let mut host = host.borrow_mut();
    host.last_event_id += 1;
    host.last_event_id
}

/// Runs the listeners of `walk.name` along the path of `walk.target` with one
/// event object. Capture listeners run from `window` down to the target, then
/// bubble listeners from the target up, the latter only while the event
/// bubbles. At the target, capture listeners run before bubble listeners. A
/// node's listeners are those registered when the walk reaches it, minus any
/// removed while an earlier listener ran. The engine is entered only when the
/// event reaches a listener.
pub(crate) fn walk(engine: &Engine, host: &Rc<RefCell<Host>>, walk: &Walk<'_>) -> Outcome {
    if !listened(&host.borrow(), walk.name, walk.flags, walk.target, walk.cut) {
        return Outcome::default();
    }
    let id = next_event_id(host);
    let mut outcome = engine.with(|ctx| run(&ctx, host, walk, id));
    outcome.entered = true;
    outcome
}

/// A walk of an event a script starts: no node is cut and the event is
/// untrusted.
pub(super) fn script_walk<'a>(
    name: &'a str,
    flags: Flags,
    target: EventTargetId,
    payload: &'a EventPayload,
) -> Walk<'a> {
    Walk {
        name,
        flags,
        target,
        cut: &[],
        payload,
        trusted: false,
    }
}

pub(super) fn run(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>, walk: &Walk<'_>, id: u64) -> Outcome {
    let Ok(callbacks) = ctx.globals().get::<_, Object>("__inca_callbacks__") else {
        return Outcome::default();
    };
    let path = path_from(&host.borrow(), walk.target);
    let created = targets_of(ctx, &path).and_then(|targets| create(ctx, walk, id, targets));
    let event = match created {
        Ok(event) => event,
        Err(err) => {
            note(ctx, host, Err(err));
            return Outcome::default();
        }
    };
    let canceled = deliver(ctx, host, walk, &path, &callbacks, &event);
    Outcome {
        canceled,
        entered: false,
    }
}

/// Files a thrown error of `result` in `Host::failures`, where the dispatcher
/// reports listener errors in the order they happened.
fn note(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>, result: JsResult<()>) {
    if let Err(err) = result {
        host.borrow_mut()
            .failures
            .push(EngineError::capture(ctx, &err));
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::event::Flags;
    use crate::tests::{
        dispatcher_with_engine, eval_json, fire_and_read, listen, listen_global, listen_with,
        node_under_root, read_out, tagged, tree_of_three,
    };
    use crate::{EventTargetId, event};
    use gpui::TestAppContext;
    use inca_gpui::EventPayload;
    use rstest::rstest;

    #[gpui::test]
    fn one_event_object_reaches_every_node(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, parent, child] = tree_of_three(&host);
        for node in [child, parent, outer] {
            listen(
                &dispatcher,
                node,
                "click",
                r"
                (e) => { globalThis.first ??= e; e.seen = (e.seen ?? 0) + 1;
                out.push([e === first, e.seen]); }
                ",
            );
        }

        let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

        assert_eq!(out, serde_json::json!([[true, 1], [true, 2], [true, 3]]));
    }

    #[gpui::test]
    fn after_dispatch_current_target_is_null_phase_0_and_stop_flags_clear(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, _, child] = tree_of_three(&host);
        listen(
            &dispatcher,
            child,
            "click",
            r"
            (e) => {
                globalThis.kept = e; e.stopPropagation(); e.preventDefault();
                out.push([e.currentTarget, e.eventPhase, e.cancelBubble]);
            }
            ",
        );
        let during = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);
        assert_eq!(during, serde_json::json!([[child, 2, true]]));

        let after = dispatcher
            .engine
            .eval::<String>(
                r"
                JSON.stringify([kept.currentTarget, kept.eventPhase, kept.cancelBubble,
                kept.defaultPrevented, kept.composedPath().length])
                ",
            )
            .unwrap();
        assert_eq!(after, "[null,0,false,true,0]");
    }

    #[gpui::test]
    fn the_path_is_fixed_when_dispatch_starts(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, parent, child] = tree_of_three(&host);
        for node in [child, parent, outer] {
            listen(
                &dispatcher,
                node,
                "click",
                &format!(
                    r"
                    (e) => {{
                        out.push([e.currentTarget, e.composedPath()]);
                        if (e.currentTarget === {child}) __inca_native__.removeChild({parent}, {child});
                    }}
                    "
                ),
            );
        }

        let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

        let path = serde_json::json!([child, parent, outer]);
        assert_eq!(
            out,
            serde_json::json!([[child, path], [parent, path], [outer, path]])
        );
    }

    #[gpui::test]
    fn each_dispatch_takes_the_next_event_id_and_an_unheard_one_takes_none(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        let record = "(e) => out.push([e.type, e.eventId])";
        listen(&dispatcher, parent, "click", record);
        listen(&dispatcher, parent, "focusin", record);
        let cx = cx.add_empty_window();

        cx.update(|window, cx| {
            let none = EventPayload::None;
            dispatcher.fire(child, "click", &none, window, cx);
            dispatcher.fire(child, "keydown", &none, window, cx);
            dispatcher.fire(child, "click", &none, window, cx);
            dispatcher.fire(child, "focusin", &none, window, cx);
        });

        let out = dispatcher
            .engine
            .eval::<String>("JSON.stringify(out)")
            .unwrap();
        assert_eq!(out, r#"[["click",1],["click",2],["focusin",3]]"#);
    }

    #[gpui::test]
    fn capture_listeners_run_root_down_before_the_bubble_listeners_run_target_up(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, parent, child] = tree_of_three(&host);
        // Bubble listeners register first, so the walk order differs from the registration order.
        for (name, node, capture) in [
            ("child", child, false),
            ("parent", parent, false),
            ("outer", outer, false),
            ("child-capture", child, true),
            ("parent-capture", parent, true),
            ("outer-capture", outer, true),
        ] {
            listen_with(&dispatcher, node, "click", (capture, false), &tagged(name));
        }

        let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

        assert_eq!(
            out,
            serde_json::json!([
                ["outer-capture", outer, 1],
                ["parent-capture", parent, 1],
                ["child-capture", child, 2],
                ["child", child, 2],
                ["parent", parent, 3],
                ["outer", outer, 3],
            ])
        );
    }

    #[gpui::test]
    fn capture_listeners_of_one_node_run_apart_from_its_bubble_listeners(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, _, child] = tree_of_three(&host);
        for (name, capture) in [("b0", false), ("c1", true), ("b2", false), ("c3", true)] {
            listen_with(&dispatcher, child, "click", (capture, false), &tagged(name));
        }

        let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

        let names: Vec<_> = out
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r[0].as_str().unwrap())
            .collect();
        assert_eq!(names, ["c1", "c3", "b0", "b2"]);
    }

    #[gpui::test]
    fn an_event_that_does_not_bubble_still_captures_through_its_ancestors(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, parent, child] = tree_of_three(&host);
        for (name, node, capture) in [
            ("outer", outer, true),
            ("outer-bubble", outer, false),
            ("parent", parent, true),
            ("parent-bubble", parent, false),
            ("child", child, true),
            ("child-bubble", child, false),
        ] {
            listen_with(&dispatcher, node, "focus", (capture, false), &tagged(name));
        }

        let out = fire_and_read(cx, &dispatcher, child, "focus", &EventPayload::None);

        assert_eq!(
            out,
            serde_json::json!([
                ["outer", outer, 1],
                ["parent", parent, 1],
                ["child", child, 2],
                ["child-bubble", child, 2],
            ])
        );
    }

    #[gpui::test]
    fn the_cut_nodes_are_skipped_in_both_passes(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, parent, child] = tree_of_three(&host);
        for (name, node, capture) in [
            ("outer-capture", outer, true),
            ("parent-capture", parent, true),
            ("child-capture", child, true),
            ("child", child, false),
            ("parent", parent, false),
            ("outer", outer, false),
        ] {
            listen_with(&dispatcher, node, "click", (capture, false), &tagged(name));
        }
        let cx = cx.add_empty_window();

        cx.update(|_, _| {
            event::walk(
                &dispatcher.engine,
                &dispatcher.host,
                &Walk {
                    name: "click",
                    flags: Flags::ALL,
                    target: EventTargetId::Node(child),
                    cut: &[parent, outer],
                    payload: &EventPayload::None,
                    trusted: true,
                },
            );
        });

        let out = dispatcher
            .engine
            .eval::<String>("JSON.stringify(out)")
            .unwrap();
        assert_eq!(
            out,
            serde_json::json!([["child-capture", child, 2], ["child", child, 2]]).to_string()
        );
    }

    #[gpui::test]
    fn a_cut_silences_document_and_window(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let outer = node_under_root(&host);
        let child = host.borrow_mut().tree.create_node("div").unwrap();
        host.borrow_mut().tree.append_child(outer, child).unwrap();
        listen(&dispatcher, child, "click", "() => out.push('child')");
        for (tag, capture) in [
            ("$document", true),
            ("$document", false),
            ("$window", false),
        ] {
            listen_global(
                &dispatcher,
                tag,
                "click",
                capture,
                "() => out.push('global')",
            );
        }
        let cx = cx.add_empty_window();

        cx.update(|_, _| {
            event::walk(
                &dispatcher.engine,
                &dispatcher.host,
                &Walk {
                    name: "click",
                    flags: Flags::ALL,
                    target: EventTargetId::Node(child),
                    cut: &[outer],
                    payload: &EventPayload::None,
                    trusted: true,
                },
            );
        });

        assert_eq!(read_out(&dispatcher), serde_json::json!(["child"]));
    }

    #[rstest]
    #[case::stop_propagation("e.stopPropagation();")]
    #[case::stop_immediate_propagation("e.stopImmediatePropagation();")]
    #[case::cancel_bubble("e.cancelBubble = true;")]
    fn a_stop_requested_before_dispatch_runs_no_listener(#[case] stop: &str) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(&dispatcher, node, "ping", "() => out.push('ran')");

        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                (() => {{
                    const e = new Event('ping');
                    {stop}
                    const returned = __inca_native__.dispatchEvent({node}, e);
                    return [returned, out];
                }})()
                "
            ),
        );

        assert_eq!(got, serde_json::json!([true, []]));
    }

    #[test]
    fn a_dispatch_no_listener_hears_clears_the_stop_flags() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();

        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                (() => {{
                    const e = new Event('ping');
                    e.stopPropagation();
                    __inca_native__.dispatchEvent({node}, e);
                    return e.cancelBubble;
                }})()
                "
            ),
        );

        assert_eq!(got, serde_json::json!(false));
    }
}
