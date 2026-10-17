// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use rstest::rstest;

#[test]
fn a_new_event_starts_idle_and_untrusted_with_every_flag_false() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();
    let got = eval_json(
        &dispatcher,
        r"
        (() => { const e = new Event('ping');
        return [e.type, e.bubbles, e.cancelable, e.composed, e.isTrusted, e.target,
            e.currentTarget, e.srcElement, e.eventPhase, e.defaultPrevented,
            e.returnValue, e.cancelBubble, e.composedPath().length,
            typeof e.timeStamp, e.constructor === Event, e instanceof Event]; })()
        ",
    );
    assert_eq!(
        got,
        serde_json::json!([
            "ping", false, false, false, false, null, null, null, 0, false, true, false, 0,
            "number", true, true
        ])
    );
}

#[test]
fn the_init_object_sets_the_flags_by_truthiness() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();
    let got = eval_json(
        &dispatcher,
        r"
        [new Event('a', { bubbles: 1, cancelable: 'x', composed: {} }),
        new Event('b', { bubbles: 0, cancelable: '' }),
        new Event('c', null), new Event('d', undefined), new Event(5),
        new Event(undefined)].map((e) => [e.type, e.bubbles, e.cancelable, e.composed])
        ",
    );
    assert_eq!(
        got,
        serde_json::json!([
            ["a", true, true, true],
            ["b", false, false, false],
            ["c", false, false, false],
            ["d", false, false, false],
            ["5", false, false, false],
            ["undefined", false, false, false]
        ])
    );
}

#[test]
fn the_constructors_refuse_a_plain_call_a_missing_type_and_a_primitive_init() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();
    let got = eval_json(
        &dispatcher,
        r"
        ['Event', 'CustomEvent'].flatMap((name) => { const C = globalThis[name];
        return [() => C('x'), () => C.call(function () {}, 'x'), () => new C(),
            () => new C('x', 5), () => new C(Symbol())].map((f) => {
            try { f(); return 'ran'; } catch (err) { return err instanceof TypeError; } }); })
        ",
    );
    assert_eq!(
        got,
        serde_json::Value::Array(vec![serde_json::json!(true); 10])
    );
}

#[test]
fn a_custom_event_carries_detail_and_inherits_from_event() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();
    let got = eval_json(
        &dispatcher,
        r"
        (() => { const d = { n: 1 };
        const a = new CustomEvent('a', { detail: d, bubbles: true });
        const b = new CustomEvent('b');
        const c = new CustomEvent('c', { detail: undefined });
        return [a.detail === d, a.bubbles, b.detail, c.detail,
            a instanceof CustomEvent, a instanceof Event,
            Object.getPrototypeOf(CustomEvent) === Event,
            a.preventDefault === new Event('x').preventDefault,
            CustomEvent.name, Event.name, Event.AT_TARGET, CustomEvent.BUBBLING_PHASE]; })()
        ",
    );
    assert_eq!(
        got,
        serde_json::json!([
            true,
            true,
            null,
            null,
            true,
            true,
            true,
            true,
            "CustomEvent",
            "Event",
            2,
            3
        ])
    );
}

#[test]
fn a_subclass_of_event_keeps_its_own_prototype() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();
    let got = eval_json(
        &dispatcher,
        r"
        (() => { class Mine extends Event { hi() { return this.type; } }
        const e = new Mine('m');
        e.preventDefault();
        return [e.hi(), e instanceof Mine, e instanceof Event, e.defaultPrevented]; })()
        ",
    );
    assert_eq!(got, serde_json::json!(["m", true, true, false]));
}

#[gpui::test]
fn host_events_of_one_kind_are_trusted_instances_of_event_sharing_a_prototype(
    cx: &mut TestAppContext,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        "click",
        r"
        (e) => { (globalThis.events ??= []).push(e);
        out.push([e instanceof Event, e.constructor === Event, e.isTrusted]); }
        ",
    );
    fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);
    let out = fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);
    assert_eq!(
        out,
        serde_json::json!([[true, true, true], [true, true, true]])
    );
    let shared = dispatcher
        .engine
        .eval::<bool>(
            r"
            const [a, b] = events;
            a !== b && Object.getPrototypeOf(a) === Object.getPrototypeOf(b)
               && a.preventDefault === b.preventDefault
               && Object.getPrototypeOf(a) !== Object.prototype
            ",
        )
        .unwrap();
    assert!(shared);
}

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
            (() => {{ const out = []; globalThis.__inca_callbacks__ = {{}};
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
fn a_dispatched_event_stays_canceled_and_gets_a_new_id_when_dispatched_again() {
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
    assert_eq!(got, serde_json::json!([[false, false], [1, 2]]));
}

#[test]
fn an_event_nobody_hears_takes_no_event_id_but_gets_its_target() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    let got = eval_json(
        &dispatcher,
        &format!(
            r"
            (() => {{ const e = new Event('ping');
            return [__inca_native__.dispatchEvent({node}, e), e.target, 'eventId' in e]; }})()
            "
        ),
    );
    assert_eq!(got, serde_json::json!([true, node, false]));
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
fn a_listener_that_throws_during_dispatch_event_is_reported_and_the_others_run() {
    let (dispatcher, host, reported) = dispatcher_with_engine();
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
fn a_listener_that_freezes_the_event_gets_a_type_error_on_redispatch() {
    let (dispatcher, host, reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        "ping",
        "(e) => { e.stopImmediatePropagation(); Object.freeze(e); }",
    );
    let got = eval_json(
        &dispatcher,
        &format!(
            r"
            (() => {{ const e = new Event('ping');
            __inca_native__.dispatchEvent({node}, e);
            const stop = e.cancelBubble;
            try {{ __inca_native__.dispatchEvent({node}, e); }}
            catch (err) {{ return [stop, err.name]; }} return [stop, 'ran']; }})()
            "
        ),
    );
    assert_eq!(got, serde_json::json!([false, "TypeError"]));
    dispatcher.report_pending();
    assert!(
        !reported.borrow().is_empty(),
        "the failed reset is reported"
    );
}

#[test]
fn a_listener_that_locks_current_target_does_not_block_the_next_dispatch() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        "ping",
        r"
        (e) => { out.push(e.currentTarget); e.stopPropagation();
        Object.defineProperty(e, 'currentTarget', { writable: false }); }
        ",
    );
    let got = eval_json(
        &dispatcher,
        &format!(
            r"
            (() => {{ const e = new Event('ping');
            __inca_native__.dispatchEvent({node}, e);
            const stop = e.cancelBubble;
            __inca_native__.dispatchEvent({node}, e);
            return [stop, out]; }})()
            "
        ),
    );
    assert_eq!(got, serde_json::json!([false, [node, node]]));
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

#[test]
fn a_frozen_event_id_lets_the_first_dispatch_through_then_throws_type_error() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        "ping",
        "(e) => Object.defineProperty(e, 'eventId', { writable: false })",
    );
    let got = eval_json(
        &dispatcher,
        &format!(
            r"
            (() => {{ const e = new Event('ping');
            return [1, 2, 3].map(() => {{
                try {{ __inca_native__.dispatchEvent({node}, e); return 'ok'; }}
                catch (err) {{ return err.name; }} }}); }})()
            "
        ),
    );
    assert_eq!(got, serde_json::json!(["ok", "TypeError", "TypeError"]));
}

#[rstest]
#[case::src_element("srcElement")]
#[case::default_prevented("defaultPrevented")]
#[case::return_value("returnValue")]
#[case::cancel_bubble("cancelBubble")]
fn an_event_accessor_descriptor_reads_enumerable_configurable(#[case] key: &str) {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        &r"
        (() => {
            const d = Object.getOwnPropertyDescriptor(Event.prototype, 'KEY');
            return [d.writable, d.enumerable, d.configurable].join();
        })()
        "
        .replace("KEY", key),
    );

    assert_eq!(got, serde_json::json!(",true,true"));
}

#[rstest]
#[case::on_the_prototype("Event.prototype")]
#[case::on_the_constructor("Event")]
fn a_phase_constant_descriptor_reads_enumerable_only(#[case] owner: &str) {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        &r"
        (() => {
            const d = Object.getOwnPropertyDescriptor(OWNER, 'AT_TARGET');
            return [d.writable, d.enumerable, d.configurable].join();
        })()
        "
        .replace("OWNER", owner),
    );

    assert_eq!(got, serde_json::json!("false,true,false"));
}

#[rstest]
#[case::event("Event")]
#[case::custom_event("CustomEvent")]
fn an_event_constructor_length_reads_one(#[case] constructor: &str) {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        &format!("Object.getOwnPropertyDescriptor({constructor}, 'length').value"),
    );

    assert_eq!(got, serde_json::json!(1));
}

#[rstest]
#[case::event("new Event('x')", "[object Event]")]
#[case::custom_event("new CustomEvent('x')", "[object CustomEvent]")]
fn an_event_string_tag_names_its_class(#[case] value: &str, #[case] want: &str) {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        &"Object.prototype.toString.call(VALUE)".replace("VALUE", value),
    );

    assert_eq!(got, serde_json::json!(want));
}

#[rstest]
#[case::event("Event.prototype", "Event")]
#[case::custom_event("CustomEvent.prototype", "CustomEvent")]
fn a_string_tag_descriptor_reads_configurable_only(#[case] owner: &str, #[case] tag: &str) {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        &r"
        (() => {
            const d = Object.getOwnPropertyDescriptor(OWNER, Symbol.toStringTag);
            return [d.value, d.writable, d.enumerable, d.configurable].join();
        })()
        "
        .replace("OWNER", owner),
    );

    assert_eq!(got, serde_json::json!(format!("{tag},false,false,true")));
}
