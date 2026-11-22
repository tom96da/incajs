// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The registrations a signal removes at its abort, seen from script.

#![allow(clippy::unwrap_used)]

use rstest::rstest;
use serde_json::json;

use crate::EventTargetId;
use crate::test_support::{Clocked, callback_ids, error_name};

/// A node, a controller `c` with its signal `s`, a log and `add(target, fn, signal, capture)`,
/// which registers a `click` listener with the signal option.
const SETUP: &str = r"
    globalThis.n = __inca_native__;
    globalThis.node = n.createNode('div');
    globalThis.c = new AbortController();
    globalThis.s = c.signal;
    globalThis.add = (target, fn, signal, capture = false) =>
        n.addEventListener(target, 'click', fn, capture, false, signal);
    ";

fn fixture() -> Clocked {
    let f = Clocked::new();
    f.engine.eval::<()>(SETUP).unwrap();
    f
}

fn node(f: &Clocked) -> u32 {
    f.engine.eval("node").unwrap()
}

fn clicks(f: &Clocked) -> Vec<u32> {
    callback_ids(
        &f.host.borrow().listeners,
        EventTargetId::Node(node(f)),
        "click",
    )
}

#[test]
fn a_registration_with_a_signal_is_removed_at_the_abort() {
    let f = fixture();
    f.engine.eval::<()>("add(node, () => {}, s);").unwrap();

    f.engine.eval::<()>("c.abort();").unwrap();

    assert!(clicks(&f).is_empty());
}

#[test]
fn a_registration_with_an_aborted_signal_registers_nothing() {
    let f = fixture();

    let got = f.json("(c.abort(), [add(node, () => {}, s), add(node, () => {})])");

    assert_eq!(got, json!([null, 1]));
}

#[rstest]
#[case::plain_object("{}")]
#[case::null("null")]
#[case::number("5")]
#[case::forged("Object.create(AbortSignal.prototype)")]
#[case::controller("new AbortController()")]
fn a_signal_argument_that_is_no_signal_throws_a_type_error(#[case] value: &str) {
    let f = fixture();

    let got = f.json(&format!(
        r"(() => {{
            try {{
                add(node, () => {{}}, {value});
            }} catch (e) {{
                return [e.name, e.message];
            }}
        }})()"
    ));

    assert_eq!(
        got,
        json!([
            "TypeError",
            "the signal option is not an AbortSignal. pass the signal of an AbortController"
        ])
    );
}

#[test]
fn an_undefined_signal_argument_registers() {
    let f = fixture();

    assert_eq!(f.json("add(node, () => {}, undefined)"), json!(1));
}

#[test]
fn one_signal_removes_listeners_of_two_nodes() {
    let f = fixture();
    f.engine
        .eval::<()>(
            r"
            globalThis.other = n.createNode('div');
            for (let i = 0; i < 5; i++) {
                add(node, () => {}, s);
                add(other, () => {}, s);
            }
            c.abort();
            ",
        )
        .unwrap();
    let other: u32 = f.engine.eval("other").unwrap();

    let left = callback_ids(
        &f.host.borrow().listeners,
        EventTargetId::Node(other),
        "click",
    );

    assert!(clicks(&f).is_empty() && left.is_empty());
}

#[test]
fn a_capture_registration_with_a_signal_is_removed_at_the_abort() {
    let f = fixture();
    f.engine
        .eval::<()>("add(node, () => {}, s, true);")
        .unwrap();

    f.engine.eval::<()>("c.abort();").unwrap();

    assert!(clicks(&f).is_empty());
}

#[test]
fn a_registration_removed_in_another_way_leaves_no_link() {
    let f = fixture();
    f.engine
        .eval::<()>(
            r"
            for (let i = 0; i < 1000; i++) {
                n.removeEventListener(node, 'click', add(node, () => {}, s));
            }
            ",
        )
        .unwrap();

    assert_eq!(f.host.borrow().listeners.links.len(), 0);
}

#[test]
fn the_free_of_a_node_unlinks_its_listeners() {
    let f = fixture();
    f.engine
        .eval::<()>("add(node, () => {}, s); n.destroyNode(node);")
        .unwrap();

    assert_eq!(f.host.borrow().listeners.links.len(), 0);
}

#[test]
fn the_removal_releases_the_callback_table_entry() {
    let f = fixture();
    let id: u32 = f.engine.eval("add(node, () => {}, s)").unwrap();

    f.engine.eval::<()>("c.abort();").unwrap();

    assert!(!crate::tests::callback_stored(&f.engine, id));
}

#[test]
fn core_hears_the_removed_ids_with_an_empty_node_list() {
    let f = fixture();
    f.engine
        .eval::<()>(
            r"
            globalThis.seen = [];
            globalThis.__inca_released__ = (nodes, ids) => seen.push([nodes, ids]);
            add(node, () => {}, s);
            c.abort();
            ",
        )
        .unwrap();

    assert_eq!(f.json("seen"), json!([[[], [1]]]));
}

#[test]
fn core_hears_the_removed_ids_before_the_abort_event() {
    let f = fixture();
    f.engine
        .eval::<()>(
            r"
            globalThis.__inca_released__ = () => log.push('released');
            add(node, () => {}, s);
            n.addEventListener(s, 'abort', () => log.push('event'));
            c.abort();
            ",
        )
        .unwrap();

    assert_eq!(f.json("log"), json!(["released", "event"]));
}

#[test]
fn a_throwing_released_hook_is_filed_and_the_abort_goes_on() {
    let f = fixture();
    f.engine
        .eval::<()>(
            r"
            globalThis.__inca_released__ = () => {
                throw new Error('hook');
            };
            add(node, () => {}, s);
            n.addEventListener(s, 'abort', () => log.push('event'));
            c.abort();
            ",
        )
        .unwrap();

    assert_eq!(f.json("log"), json!(["event"]));
    assert_eq!(f.host.borrow().failures.len(), 1);
}

#[test]
fn the_removals_run_before_the_abort_event() {
    let f = fixture();
    f.engine
        .eval::<()>(
            r"
            add(node, () => log.push('click'), s);
            n.addEventListener(s, 'abort', () => n.dispatchEvent(node, new Event('click')));
            c.abort();
            ",
        )
        .unwrap();

    assert_eq!(f.json("log"), json!([]));
}

#[test]
fn a_listener_added_during_the_event_with_another_signal_is_removed_by_its_abort() {
    let f = fixture();
    f.engine
        .eval::<()>(
            r"
            globalThis.other = new AbortController();
            n.addEventListener(s, 'abort', () => add(node, () => {}, other.signal));
            c.abort();
            other.abort();
            ",
        )
        .unwrap();

    assert!(clicks(&f).is_empty());
}

#[test]
fn a_listener_on_another_dependent_unpins_it_when_the_abort_empties_it() {
    let f = fixture();
    f.engine
        .eval::<()>(
            r"
            (() => {
                globalThis.keep = new AbortController().signal;
                const dependent = AbortSignal.any([
                    new AbortController().signal,
                    globalThis.keep,
                ]);
                n.addEventListener(dependent, 'abort', () => {}, false, false, s);
                globalThis.w = new WeakRef(dependent);
            })();
            c.abort();
            ",
        )
        .unwrap();

    for _ in 0..2 {
        f.engine.with(|ctx| ctx.run_gc());
        drop(f.engine.run_jobs());
    }

    assert_eq!(f.json("w.deref() === undefined"), json!(true));
}

#[test]
fn a_listener_whose_signal_option_is_a_dependent_is_removed_after_collection() {
    let f = fixture();
    f.engine
        .eval::<()>("add(node, () => {}, AbortSignal.any([s, new AbortController().signal]));")
        .unwrap();

    for _ in 0..2 {
        f.engine.with(|ctx| ctx.run_gc());
        drop(f.engine.run_jobs());
    }
    f.engine.eval::<()>("c.abort();").unwrap();

    assert!(clicks(&f).is_empty());
}

#[test]
fn a_listener_added_with_two_signals_in_turn_leaves_with_each_signal_alone() {
    let f = fixture();
    f.engine
        .eval::<()>(
            r"
            globalThis.second = new AbortController();
            const listener = () => {};
            add(node, listener, s);
            add(node, listener, second.signal);
            second.abort();
            ",
        )
        .unwrap();
    let after_second = clicks(&f).len();

    f.engine.eval::<()>("c.abort();").unwrap();

    assert_eq!([after_second, clicks(&f).len()], [1, 0]);
}

#[test]
fn a_listener_added_without_a_signal_stays_after_the_abort_of_a_later_registration() {
    let f = fixture();
    f.engine
        .eval::<()>(
            r"
            const listener = () => {};
            add(node, listener);
            add(node, listener, s);
            c.abort();
            ",
        )
        .unwrap();

    assert_eq!(clicks(&f).len(), 1);
}

#[test]
fn a_listener_that_removes_itself_leaves_no_link_after_it_runs() {
    let f = fixture();
    f.engine
        .eval::<()>(
            r"
            const id = add(node, () => {
                n.removeEventListener(node, 'click', id);
                log.push('ran');
            }, s);
            n.dispatchEvent(node, new Event('click'));
            ",
        )
        .unwrap();

    assert_eq!(f.json("log"), json!(["ran"]));
    assert_eq!(f.host.borrow().listeners.links.len(), 0);
}

/// A dependent signal with two linked listeners, the first removed.
fn dependent_with_one_link_left(f: &Clocked) {
    f.engine
        .eval::<()>(
            r"
            (() => {
                const dependent = AbortSignal.any([s]);
                globalThis.first = add(node, () => {}, dependent);
                globalThis.second = add(node, () => {}, dependent);
                globalThis.w = new WeakRef(dependent);
            })();
            n.removeEventListener(node, 'click', first);
            ",
        )
        .unwrap();
}

fn collect(f: &Clocked) {
    for _ in 0..2 {
        f.engine.with(|ctx| ctx.run_gc());
        drop(f.engine.run_jobs());
    }
}

#[test]
fn the_pin_stays_while_a_link_is_left() {
    let f = fixture();
    dependent_with_one_link_left(&f);

    collect(&f);

    assert_eq!(f.json("w.deref() !== undefined"), json!(true));
}

#[test]
fn the_pin_ends_with_the_last_link() {
    let f = fixture();
    dependent_with_one_link_left(&f);
    f.engine
        .eval::<()>("n.removeEventListener(node, 'click', second);")
        .unwrap();

    collect(&f);

    assert_eq!(f.json("w.deref() === undefined"), json!(true));
}

#[test]
fn an_aborted_signal_with_a_freed_node_target_still_throws_the_node_error() {
    let f = fixture();
    f.engine
        .eval::<()>("c.abort(); n.destroyNode(node);")
        .unwrap();

    assert_eq!(error_name(&f.engine, "add(node, () => {}, s)"), "TypeError");
}

#[rstest]
#[case::remove_node("n.removeNode(node);")]
#[case::destroy_node("n.destroyNode(node);")]
fn freeing_a_node_unpins_the_dependent_its_listener_names(#[case] free: &str) {
    let f = fixture();
    f.engine
        .eval::<()>(&format!(
            r"
            (() => {{
                const dependent = AbortSignal.any([s]);
                add(node, () => {{}}, dependent);
                globalThis.w = new WeakRef(dependent);
            }})();
            {free}
            "
        ))
        .unwrap();

    for _ in 0..2 {
        f.engine.with(|ctx| ctx.run_gc());
        drop(f.engine.run_jobs());
    }

    assert_eq!(f.json("w.deref() === undefined"), json!(true));
}

#[test]
fn a_collected_signal_drops_its_links() {
    let f = fixture();
    f.engine
        .eval::<()>(
            r"
            (() => {
                add(node, () => {}, new AbortController().signal);
            })();
            ",
        )
        .unwrap();

    f.engine.with(|ctx| ctx.run_gc());
    drop(f.engine.run_jobs());

    assert_eq!(f.host.borrow().listeners.links.len(), 0);
    assert_eq!(clicks(&f).len(), 1);
}

#[test]
fn a_timeout_signal_removes_its_listener_when_the_timer_runs() {
    let f = fixture();
    f.engine
        .eval::<()>("add(node, () => {}, AbortSignal.timeout(5));")
        .unwrap();

    f.advance(5);

    assert!(clicks(&f).is_empty());
}
