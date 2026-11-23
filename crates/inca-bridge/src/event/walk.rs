// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::rc::Rc;

use inca_document::{EventPayload, Outcome, Walk, deliver, path_from};
use inca_jsenv::{Engine, EngineError};
use inca_node::NodeId;
use rquickjs::{Ctx, Result as JsResult};

use super::construct::create;
use super::walk_listeners::JsInvoke;
use crate::bindings::Host;
use inca_document::DocumentState;
use inca_document::EventTargetId;
use inca_document::Flags;

pub(crate) use super::walk_click::click;
pub(crate) use super::walk_script::{dispatch_event, lose_focus};
pub(crate) use super::walk_targets::fire_abort;
pub(super) use super::walk_targets::targets_of;

/// Whether a listener of `name` exists that an event with `flags` fired at
/// `target` reaches. Nodes in `cut` stay silent.
pub(crate) fn listened(
    host: &DocumentState,
    name: &str,
    flags: Flags,
    target: EventTargetId,
    cut: &[NodeId],
) -> bool {
    inca_document::listened(&host.tree, &host.listeners, name, flags, target, cut)
}

pub(super) fn next_event_id(host: &Rc<Host>) -> u64 {
    inca_document::next_event_id(&mut host.document.borrow_mut().last_event_id)
}

/// Runs the listeners of `walk.name` along the path of `walk.target` with one
/// event object. Capture listeners run from `window` down to the target, then
/// bubble listeners from the target up, the latter only while the event
/// bubbles. At the target, capture listeners run before bubble listeners. A
/// node's listeners are those registered when the walk reaches it, minus any
/// removed while an earlier listener ran. The engine is entered only when the
/// event reaches a listener.
pub(crate) fn walk(engine: &Engine, host: &Rc<Host>, walk: &Walk<'_>) -> Outcome {
    if !listened(
        &host.document.borrow(),
        walk.name,
        walk.flags,
        walk.target,
        walk.cut,
    ) {
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

pub(super) fn run(ctx: &Ctx<'_>, host: &Rc<Host>, walk: &Walk<'_>, id: u64) -> Outcome {
    let path = path_from(&host.document.borrow().tree, walk.target);
    let created = targets_of(ctx, &path).and_then(|targets| create(ctx, walk, id, targets));
    let event = match created {
        Ok(event) => event,
        Err(err) => {
            note(ctx, host, Err(err));
            return Outcome::default();
        }
    };
    let canceled = deliver(
        walk,
        &path,
        &JsInvoke {
            ctx,
            host,
            event: &event,
        },
    );
    Outcome {
        canceled,
        entered: false,
    }
}

/// Files a thrown error of `result` in `JsState::failures`, where the dispatcher
/// reports listener errors in the order they happened.
pub(super) fn note(ctx: &Ctx<'_>, host: &Rc<Host>, result: JsResult<()>) {
    if let Err(err) = result {
        host.js
            .borrow_mut()
            .failures
            .push(EngineError::capture(ctx, &err));
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::event;
    use crate::tests::{
        dispatcher_with_engine, eval_json, fire_and_read, listen, listen_global, listen_with,
        node_under_root, read_out, tagged, tree_of_three,
    };
    use inca_document::EventPayload;
    use inca_document::EventTargetId;
    use inca_document::Flags;
    use rstest::rstest;

    #[test]
    fn one_event_object_reaches_every_node() {
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

        let out = fire_and_read(&dispatcher, child, "click", &EventPayload::None);

        assert_eq!(out, serde_json::json!([[true, 1], [true, 2], [true, 3]]));
    }

    #[test]
    fn after_dispatch_current_target_is_null_phase_0_and_stop_flags_clear() {
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
        let during = fire_and_read(&dispatcher, child, "click", &EventPayload::None);
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

    #[test]
    fn the_path_is_fixed_when_dispatch_starts() {
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

        let out = fire_and_read(&dispatcher, child, "click", &EventPayload::None);

        let path = serde_json::json!([child, parent, outer]);
        assert_eq!(
            out,
            serde_json::json!([[child, path], [parent, path], [outer, path]])
        );
    }

    #[test]
    fn each_dispatch_takes_the_next_event_id_and_an_unheard_one_takes_none() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        let record = "(e) => out.push([e.type, e.eventId])";
        listen(&dispatcher, parent, "click", record);
        listen(&dispatcher, parent, "focusin", record);
        let none = EventPayload::None;
        dispatcher.fire(child, "click", &none);
        dispatcher.fire(child, "keydown", &none);
        dispatcher.fire(child, "click", &none);
        dispatcher.fire(child, "focusin", &none);

        let out = dispatcher
            .engine
            .eval::<String>("JSON.stringify(out)")
            .unwrap();
        assert_eq!(out, r#"[["click",1],["click",2],["focusin",3]]"#);
    }

    #[test]
    fn the_cut_nodes_are_skipped_in_both_passes() {
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

        let out = dispatcher
            .engine
            .eval::<String>("JSON.stringify(out)")
            .unwrap();
        assert_eq!(
            out,
            serde_json::json!([["child-capture", child, 2], ["child", child, 2]]).to_string()
        );
    }

    #[test]
    fn a_cut_silences_the_document_and_window() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let outer = node_under_root(&host);
        let child = host.document.borrow_mut().tree.create_node("div").unwrap();
        host.document
            .borrow_mut()
            .tree
            .append_child(outer, child)
            .unwrap();
        let document = host.document.borrow().document();
        listen(&dispatcher, child, "click", "() => out.push('child')");
        listen_with(
            &dispatcher,
            document,
            "click",
            (true, false),
            "() => out.push('global')",
        );
        listen(&dispatcher, document, "click", "() => out.push('global')");
        listen_global(
            &dispatcher,
            "$window",
            "click",
            false,
            "() => out.push('global')",
        );
        event::walk(
            &dispatcher.engine,
            &dispatcher.host,
            &Walk {
                name: "click",
                flags: Flags::ALL,
                target: EventTargetId::Node(child),
                cut: &[outer, document],
                payload: &EventPayload::None,
                trusted: true,
            },
        );

        assert_eq!(read_out(&dispatcher), serde_json::json!(["child"]));
    }

    #[rstest]
    #[case::stop_propagation("e.stopPropagation();")]
    #[case::stop_immediate_propagation("e.stopImmediatePropagation();")]
    #[case::cancel_bubble("e.cancelBubble = true;")]
    fn a_stop_requested_before_dispatch_runs_no_listener(#[case] stop: &str) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.document.borrow_mut().tree.create_node("div").unwrap();
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
        let node = host.document.borrow_mut().tree.create_node("div").unwrap();

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
