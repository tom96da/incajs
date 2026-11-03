// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `dispatchEvent` and `click` natives that start a walk from a script.

use std::cell::RefCell;
use std::rc::Rc;

use inca_gpui::{EventPayload, MousePayload, NodeId};
use rquickjs::{Class, Ctx, Exception, Object, Result as JsResult, Value};

use super::listeners::{deliver, end_dispatch};
use super::{IncaEvent, listened, next_event_id, path_from, run, script_walk, targets_of};
use crate::EventTargetId;
use crate::bindings::Host;

/// Dispatches a script-built `event` at `target` and returns `false` when a
/// listener canceled it. Listener errors go to `Host::failures`.
///
/// # Errors
///
/// Throws a `TypeError` when `event` is not an event object of this host, and
/// an `InvalidStateError` while `event` is being dispatched.
/// Throws what `__inca_targets__` throws.
pub(crate) fn dispatch_event<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<RefCell<Host>>,
    target: EventTargetId,
    event: &Value<'js>,
) -> JsResult<bool> {
    let event = Class::<IncaEvent>::from_value(event).map_err(|_| {
        Exception::throw_type(
            ctx,
            concat!(
                "dispatchEvent needs an event object, ",
                "so pass one made with new Event(type)"
            ),
        )
    })?;
    let record = event.borrow();
    let state = &record.state;
    if state.dispatching.get() {
        return Err(Exception::throw_dom(
            ctx,
            "InvalidStateError",
            concat!(
                "the event is already being dispatched, ",
                "so dispatch it again after its listeners return"
            ),
        ));
    }
    let path = path_from(&host.borrow(), target);
    let name = state.name.borrow().clone();
    let walk = script_walk(&name, state.flags.get(), target, &EventPayload::None);
    let targets = targets_of(ctx, &path)?;
    drop(record.held.target.replace(targets.first().cloned()));
    state.trusted.set(false);
    if listened(&host.borrow(), walk.name, walk.flags, target, walk.cut) {
        let callbacks = ctx
            .globals()
            .get::<_, Object>("__inca_callbacks__")
            .or_else(|_| Object::new(ctx.clone()))?;
        state.event_id.set(Some(next_event_id(host)));
        drop(record.held.path.replace(targets));
        // Last, so a failure above leaves the event free to dispatch again.
        state.dispatching.set(true);
        deliver(ctx, host, &walk, &path, &callbacks, &event);
    } else {
        end_dispatch(&record);
    }
    Ok(!state.canceled.get())
}

/// Runs an untrusted `click` at `target`. A disabled button or input, a node that left the tree
/// and a node whose own click is still running ignore the call.
pub(crate) fn click(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>, target: NodeId) {
    {
        let mut host = host.borrow_mut();
        let Some(node) = host.tree.get(target) else {
            return;
        };
        if inca_gpui::is_disabled_control(node) || !host.clicking.insert(target) {
            return;
        }
    }
    let payload = EventPayload::Mouse(MousePayload::keyboard_click(gpui::Modifiers::none()));
    let walk = script_walk(
        "click",
        crate::dispatch::spec_of("click").flags,
        EventTargetId::Node(target),
        &payload,
    );
    if listened(&host.borrow(), walk.name, walk.flags, walk.target, walk.cut) {
        run(ctx, host, &walk, next_event_id(host));
    }
    host.borrow_mut().clicking.remove(&target);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::EventDispatcher;
    use crate::tests::{
        dispatcher_with_engine, eval_json, fire_and_read, listen, listen_global, listen_with,
        node_under_root, read_out, tree_of_three,
    };
    use gpui::TestAppContext;
    use inca_gpui::{EventPayload, NodeId};

    #[test]
    fn dispatch_event_walks_capture_target_bubble_and_returns_true_when_not_canceled() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, parent, child] = tree_of_three(&host);
        listen_with(
            &dispatcher,
            outer,
            "ping",
            (true, false),
            "(e) => out.push(['cap', e.currentTarget, e.eventPhase, e.target, e.isTrusted])",
        );
        listen(
            &dispatcher,
            child,
            "ping",
            "(e) => out.push(['at', e.currentTarget, e.eventPhase])",
        );
        listen(
            &dispatcher,
            parent,
            "ping",
            "(e) => out.push(['up', e.currentTarget, e.eventPhase])",
        );
        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                (() => {{ const e = new Event('ping', {{ bubbles: true }});
                const r = __inca_native__.dispatchEvent({child}, e);
                return [r, out, e.target, e.currentTarget, e.eventPhase, e.composedPath().length]; }})()
                "
            ),
        );
        assert_eq!(
            got,
            serde_json::json!([
                true,
                [
                    ["cap", outer, 1, child, false],
                    ["at", child, 2],
                    ["up", parent, 3]
                ],
                child,
                null,
                0,
                0
            ])
        );
    }

    #[test]
    fn dispatch_event_skips_the_ancestors_of_an_event_that_does_not_bubble() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        listen(&dispatcher, child, "ping", "() => out.push('child')");
        listen(&dispatcher, parent, "ping", "() => out.push('parent')");
        let got = eval_json(
            &dispatcher,
            &format!("(__inca_native__.dispatchEvent({child}, new Event('ping')), out)"),
        );
        assert_eq!(got, serde_json::json!(["child"]));
    }

    #[test]
    fn a_dispatched_event_calls_the_function_stored_under_the_returned_id() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                (() => {{ const out = [];
                const first = __inca_native__.addEventListener({node}, 'ping');
                const second = __inca_native__.addEventListener({node}, 'ping');
                __inca_callbacks__[first] = () => out.push('first');
                __inca_callbacks__[second] = () => out.push('second');
                __inca_native__.dispatchEvent({node}, new Event('ping'));
                return [first, second, out]; }})()
                "
            ),
        );
        assert_eq!(got, serde_json::json!([0, 1, ["first", "second"]]));
    }

    #[test]
    fn dispatch_event_returns_false_for_a_canceled_cancelable_event_only() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(&dispatcher, node, "ping", "(e) => e.preventDefault()");
        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                [true, false].map((cancelable) => {{ const e = new Event('ping', {{ cancelable }});
                return [__inca_native__.dispatchEvent({node}, e), e.defaultPrevented]; }})
                "
            ),
        );
        assert_eq!(got, serde_json::json!([[false, true], [true, false]]));
    }

    #[test]
    fn a_canceled_event_stays_canceled_when_dispatched_again() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "ping",
            "(e) => { out.push(e.eventId); e.preventDefault(); }",
        );
        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                (() => {{ const e = new Event('ping', {{ cancelable: true }});
                const r = [__inca_native__.dispatchEvent({node}, e),
                    __inca_native__.dispatchEvent({node}, e)];
                return [r, out]; }})()
                "
            ),
        );
        assert_eq!(got[0], serde_json::json!([false, false]));
    }

    #[test]
    fn a_redispatched_event_gets_a_new_event_id() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "ping",
            "(e) => { out.push(e.eventId); e.preventDefault(); }",
        );
        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                (() => {{ const e = new Event('ping', {{ cancelable: true }});
                const r = [__inca_native__.dispatchEvent({node}, e),
                    __inca_native__.dispatchEvent({node}, e)];
                return [r, out]; }})()
                "
            ),
        );
        assert_eq!(got[1], serde_json::json!([1, 2]));
    }

    #[test]
    fn an_event_nobody_hears_takes_no_event_id_but_gets_its_target() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                (() => {{
                    const e = new Event('ping');
                    const handled = __inca_native__.dispatchEvent({node}, e);
                    return [handled, e.target, e.eventId === undefined];
                }})()
                "
            ),
        );
        assert_eq!(got, serde_json::json!([true, node, true]));
        assert_eq!(host.borrow().last_event_id, 0);
    }

    #[test]
    fn dispatch_event_while_dispatching_throws_invalid_state_and_the_walk_continues() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "ping",
            &format!(
                r"
                (e) => {{ try {{ __inca_native__.dispatchEvent({node}, e); out.push('ran'); }}
                catch (err) {{ out.push([err.name, e.eventPhase]); }} }}
                "
            ),
        );
        let got = eval_json(
            &dispatcher,
            &format!("(__inca_native__.dispatchEvent({node}, new Event('ping')), out)"),
        );
        assert_eq!(got, serde_json::json!([["InvalidStateError", 2]]));
    }

    #[test]
    fn a_nested_dispatch_throws_an_error_that_is_a_domexception_instance() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "ping",
            &format!(
                r"
                (e) => {{
                    try {{ __inca_native__.dispatchEvent({node}, e); }}
                    catch (err) {{
                        out.push([typeof DOMException, err instanceof DOMException]);
                    }}
                }}
                "
            ),
        );
        let got = eval_json(
            &dispatcher,
            &format!("(__inca_native__.dispatchEvent({node}, new Event('ping')), out)"),
        );
        assert_eq!(got, serde_json::json!([["function", true]]));
    }

    #[test]
    fn dispatch_event_throws_for_anything_that_is_not_an_event_of_this_host() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                [undefined, null, 5, 'click', {{ type: 'click' }}, Object.create(new Event('x')),
                Event.prototype].map((v) => {{
                  try {{ __inca_native__.dispatchEvent({node}, v); return 'ran'; }}
                  catch (err) {{ return err instanceof TypeError; }}
                }})
                "
            ),
        );
        assert_eq!(
            got,
            serde_json::Value::Array(vec![serde_json::json!(true); 7])
        );
    }

    #[test]
    fn dispatch_event_on_an_unknown_node_throws_a_type_error() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let got = eval_json(
            &dispatcher,
            r"
            (() => { try { __inca_native__.dispatchEvent(99, new Event('x')); return 'ran'; }
            catch (err) { return err instanceof TypeError; } })()
            ",
        );
        assert_eq!(got, serde_json::json!(true));
    }

    #[test]
    fn a_listener_that_throws_during_dispatch_event_lets_the_others_run() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "ping",
            "() => { throw new Error('boom'); }",
        );
        listen(&dispatcher, node, "ping", "() => out.push('second')");
        let got = eval_json(
            &dispatcher,
            &format!("[__inca_native__.dispatchEvent({node}, new Event('ping')), out]"),
        );
        assert_eq!(got, serde_json::json!([true, ["second"]]));
    }

    #[test]
    fn a_listener_that_throws_during_dispatch_event_is_reported_once_idle() {
        let (dispatcher, host, reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "ping",
            "() => { throw new Error('boom'); }",
        );
        listen(&dispatcher, node, "ping", "() => out.push('second')");
        let _ = eval_json(
            &dispatcher,
            &format!("[__inca_native__.dispatchEvent({node}, new Event('ping')), out]"),
        );
        assert!(
            reported.borrow().is_empty(),
            "reported once the engine is idle"
        );

        dispatcher.report_pending();
        assert_eq!(reported.borrow().len(), 1);
        assert_eq!(reported.borrow()[0].message(), "Error: boom");
        dispatcher.report_pending();
        assert_eq!(reported.borrow().len(), 1);
    }

    #[gpui::test]
    fn errors_are_reported_in_the_order_they_happened_across_nested_dispatches(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "click",
            &format!(
                r"
                () => {{
                    __inca_native__.dispatchEvent({node}, new Event('ping'));
                    throw new Error('outer');
                }}
                "
            ),
        );
        listen(
            &dispatcher,
            node,
            "ping",
            "() => { throw new Error('inner'); }",
        );
        fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);
        let messages: Vec<_> = reported
            .borrow()
            .iter()
            .map(|e| e.message().to_owned())
            .collect();
        assert_eq!(messages, ["Error: inner", "Error: outer"]);
    }

    #[test]
    fn a_frozen_event_dispatches_again() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "ping",
            r"
            (e) => {
                Object.freeze(e);
                out.push(e.eventPhase);
            }
            ",
        );

        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                (() => {{
                    const e = new Event('ping');
                    __inca_native__.dispatchEvent({node}, e);
                    __inca_native__.dispatchEvent({node}, e);
                    return out;
                }})()
                "
            ),
        );

        assert_eq!(got, serde_json::json!([2, 2]));
    }

    #[gpui::test]
    fn a_trusted_host_event_kept_by_a_listener_is_untrusted_when_dispatched_again(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "click",
            "(e) => { out.push(e.isTrusted); globalThis.kept ??= e; }",
        );
        fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);
        let got = eval_json(
            &dispatcher,
            &format!("(__inca_native__.dispatchEvent({node}, kept), out)"),
        );
        assert_eq!(got, serde_json::json!([true, false]));
    }

    #[test]
    fn the_stop_state_of_an_outer_event_survives_a_nested_dispatch() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        listen(
            &dispatcher,
            child,
            "ping",
            &format!(
                r"
                (e) => {{
                    e.stopPropagation();
                   __inca_native__.dispatchEvent({child}, new Event('inner'));
                   out.push(['child', e.cancelBubble]);
                }}
                "
            ),
        );
        listen(&dispatcher, parent, "ping", "() => out.push('parent')");
        listen(&dispatcher, child, "inner", "() => out.push('inner')");
        let got = eval_json(
            &dispatcher,
            &format!(
                "(__inca_native__.dispatchEvent({child}, new Event('ping', {{ bubbles: true }})), out)"
            ),
        );
        assert_eq!(got, serde_json::json!(["inner", ["child", true]]));
    }

    #[test]
    fn a_custom_event_shows_its_detail_to_the_listener_it_is_dispatched_to() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "ping",
            "(e) => out.push([e.detail, e instanceof CustomEvent, e.isTrusted])",
        );
        let got = eval_json(
            &dispatcher,
            &format!(
                "(__inca_native__.dispatchEvent({node}, new CustomEvent('ping', {{ detail: {{ n: 1 }} }})), out)"
            ),
        );
        assert_eq!(got, serde_json::json!([[{"n": 1}, true, false]]));
    }

    #[test]
    fn dispatch_event_of_a_click_reaches_a_disabled_button_and_its_ancestors() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, _] = tree_of_three(&host);
        let button = host.borrow_mut().tree.create_node("button").unwrap();
        host.borrow_mut().tree.append_child(parent, button).unwrap();
        host.borrow_mut()
            .tree
            .set_attribute(button, "disabled", true)
            .unwrap();
        listen(&dispatcher, button, "click", "() => out.push('button')");
        listen_with(
            &dispatcher,
            parent,
            "click",
            (true, false),
            "() => out.push('capture')",
        );
        listen(&dispatcher, parent, "click", "() => out.push('bubble')");
        let got = eval_json(
            &dispatcher,
            &format!(
                "(__inca_native__.dispatchEvent({button}, new Event('click', {{ bubbles: true }})), out)"
            ),
        );
        assert_eq!(got, serde_json::json!(["capture", "button", "bubble"]));
    }

    /// Dispatches a bubbling `ping` at `target`, a JS expression, with a listener on the node,
    /// document and window, and returns what each heard as `[label, target, currentTarget,
    /// eventPhase]`.
    fn ping_from(dispatcher: &EventDispatcher, node: NodeId, target: &str) -> serde_json::Value {
        let record = |label: &str| {
            format!("(e) => out.push(['{label}', e.target, e.currentTarget, e.eventPhase])")
        };
        listen(dispatcher, node, "ping", &record("node"));
        listen_global(dispatcher, "$document", "ping", false, &record("document"));
        listen_global(dispatcher, "$window", "ping", false, &record("window"));
        dispatcher
            .engine
            .eval::<()>(&format!(
                "__inca_native__.dispatchEvent({target}, new Event('ping', {{ bubbles: true }}));"
            ))
            .unwrap();
        read_out(dispatcher)
    }

    #[test]
    fn element_dispatch_bubbles_through_document_and_window() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = node_under_root(&host);

        let out = ping_from(&dispatcher, node, &node.to_string());

        assert_eq!(
            out,
            serde_json::json!([
                ["node", node, node, 2],
                ["document", node, "$document", 3],
                ["window", node, "$window", 3],
            ])
        );
    }

    #[test]
    fn document_dispatch_bubbles_to_window() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = node_under_root(&host);

        let out = ping_from(&dispatcher, node, "'$document'");

        assert_eq!(
            out,
            serde_json::json!([
                ["document", "$document", "$document", 2],
                ["window", "$document", "$window", 3],
            ])
        );
    }

    #[test]
    fn window_dispatch_stays_at_window() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = node_under_root(&host);

        let out = ping_from(&dispatcher, node, "'$window'");

        assert_eq!(
            out,
            serde_json::json!([["window", "$window", "$window", 2]])
        );
    }

    #[test]
    fn click_runs_an_untrusted_bubbling_click_with_the_pointer_fields_of_a_key_click() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        listen(
            &dispatcher,
            child,
            "click",
            r"
            (e) => out.push([
                e.type, e.isTrusted, e.bubbles, e.cancelable, e.target,
                e.currentTarget, e.pointerId, e.pointerType, e.button, e.buttons, e.detail,
                e.ctrlKey, e.eventId
            ])
            ",
        );
        listen(
            &dispatcher,
            parent,
            "click",
            "(e) => out.push(['parent', e.currentTarget, e.eventPhase])",
        );
        let got = eval_json(
            &dispatcher,
            &format!("(__inca_native__.click({child}), out)"),
        );
        assert_eq!(
            got,
            serde_json::json!([
                [
                    "click", false, true, true, child, child, -1, "", 0, 0, 0, false, 1
                ],
                ["parent", parent, 3]
            ])
        );
    }

    #[test]
    fn click_on_a_disabled_button_does_nothing_and_on_an_enabled_one_runs() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let button = host.borrow_mut().tree.create_node("button").unwrap();
        listen(&dispatcher, button, "click", "() => out.push('click')");
        host.borrow_mut()
            .tree
            .set_attribute(button, "disabled", true)
            .unwrap();
        let disabled = eval_json(
            &dispatcher,
            &format!("(__inca_native__.click({button}), out.length)"),
        );
        host.borrow_mut()
            .tree
            .remove_attribute(button, "disabled")
            .unwrap();
        let enabled = eval_json(
            &dispatcher,
            &format!("(__inca_native__.click({button}), out.length)"),
        );
        assert_eq!(
            (disabled, enabled),
            (serde_json::json!(0), serde_json::json!(1))
        );
    }

    #[test]
    fn click_inside_the_nodes_own_click_does_nothing_and_a_later_click_runs() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "click",
            &format!("() => {{ out.push('click'); __inca_native__.click({node}); }}"),
        );
        let got = eval_json(
            &dispatcher,
            &format!("(__inca_native__.click({node}), __inca_native__.click({node}), out)"),
        );
        assert_eq!(got, serde_json::json!(["click", "click"]));
        assert!(host.borrow().clicking.is_empty());
    }

    #[test]
    fn click_on_one_node_inside_the_click_of_another_runs() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        listen(
            &dispatcher,
            parent,
            "click",
            &format!(
                r"
                (e) => {{
                    out.push(e.target);
                    if (e.target === {parent}) __inca_native__.click({child});
                }}
                "
            ),
        );
        listen(&dispatcher, child, "click", "() => {}");
        let got = eval_json(
            &dispatcher,
            &format!("(__inca_native__.click({parent}), out)"),
        );
        assert_eq!(got, serde_json::json!([parent, child]));
    }

    #[test]
    fn click_on_an_unheard_node_takes_no_event_id() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        dispatcher
            .engine
            .eval::<()>(&format!("__inca_native__.click({node});"))
            .unwrap();
        assert_eq!(host.borrow().last_event_id, 0);
    }

    #[test]
    fn click_on_an_unknown_node_throws_a_type_error() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let caught: bool = dispatcher
            .engine
            .eval(
                r"
                let caught = false;
                try {
                    __inca_native__.click(99);
                } catch (e) {
                    caught = e instanceof TypeError;
                }
                caught;
                ",
            )
            .unwrap();
        assert!(caught);
    }

    #[test]
    fn click_reports_a_throwing_listener_through_the_pending_failures() {
        let (dispatcher, host, reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "click",
            "() => { throw new Error('boom'); }",
        );
        dispatcher
            .engine
            .eval::<()>(&format!("__inca_native__.click({node});"))
            .unwrap();
        dispatcher.report_pending();
        assert_eq!(reported.borrow()[0].message(), "Error: boom");
    }
}
