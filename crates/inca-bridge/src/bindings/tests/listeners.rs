// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[test]
fn registering_the_same_callback_twice_keeps_one_entry() {
    let mut listeners = EventListeners::default();
    listeners.register(1, "click", 7);
    listeners.register(1, "click", 7);

    assert_eq!(listeners.callbacks_for(1, "click"), &[7]);
}

#[test]
fn distinct_callbacks_on_one_event_stack() {
    let mut listeners = EventListeners::default();
    listeners.register(1, "click", 7);
    listeners.register(1, "click", 8);

    assert_eq!(listeners.callbacks_for(1, "click"), &[7, 8]);
}

#[test]
fn a_registration_keeps_its_phase_and_passivity_and_the_first_one_wins() {
    let mut listeners = EventListeners::default();
    listeners.register_with(1, "wheel", 7, true, true);
    listeners.register_with(1, "wheel", 7, false, false);
    listeners.register(1, "wheel", 8);

    assert_eq!(
        listeners.listeners_for(1, "wheel"),
        &[
            Listener {
                callback: 7,
                capture: true,
                passive: true
            },
            Listener {
                callback: 8,
                capture: false,
                passive: false
            },
        ]
    );
}

#[test]
fn add_event_listener_takes_optional_capture_and_passive_flags() {
    let (engine, host) = engine_with_bindings();
    engine
        .eval::<()>(
            "const n = __inca_native__.createNode('div'); \
             __inca_native__.addEventListener(n, 'a'); \
             __inca_native__.addEventListener(n, 'a', true); \
             __inca_native__.addEventListener(n, 'a', false, true); \
             __inca_native__.addEventListener(n, 'a', true, true); \
             __inca_native__.removeEventListener(n, 'a', 3);",
        )
        .unwrap();
    let host = host.borrow();
    let flags: Vec<_> = host
        .listeners
        .listeners_for(1, "a")
        .iter()
        .map(|l| (l.callback, l.capture, l.passive))
        .collect();
    assert_eq!(
        flags,
        [(0, false, false), (1, true, false), (2, false, true)]
    );
}

#[test]
fn add_hands_out_ids_from_zero_in_call_order_across_nodes_and_events() {
    let mut listeners = EventListeners::default();

    assert_eq!(listeners.add(1, "click", false, false), Some(0));
    assert_eq!(listeners.add(2, "click", true, false), Some(1));
    assert_eq!(listeners.add(1, "focus", false, true), Some(2));
    assert_eq!(listeners.callbacks_for(1, "click"), &[0]);
    assert_eq!(listeners.callbacks_for(2, "click"), &[1]);
    assert_eq!(listeners.callbacks_for(1, "focus"), &[2]);
    assert_eq!(
        listeners.listeners_for(1, "focus"),
        &[Listener {
            callback: 2,
            capture: false,
            passive: true
        }]
    );
}

#[test]
fn add_never_reuses_an_id_after_unregister_or_release() {
    let mut listeners = EventListeners::default();
    let first = listeners.add(1, "click", false, false).unwrap();
    listeners.unregister(1, "click", first);
    let second = listeners.add(1, "click", false, false).unwrap();
    listeners.release(1);

    assert_eq!((first, second), (0, 1));
    assert_eq!(listeners.add(1, "click", false, false), Some(2));
}

#[test]
fn add_returns_none_and_registers_nothing_once_the_ids_run_out() {
    let mut listeners = EventListeners {
        next_callback: u32::MAX - 1,
        ..EventListeners::default()
    };

    assert_eq!(listeners.add(1, "click", false, false), Some(u32::MAX - 1));
    assert_eq!(listeners.add(1, "focus", false, false), None);
    assert_eq!(listeners.add(1, "focus", false, false), None);
    assert!(listeners.callbacks_for(1, "focus").is_empty());
}

#[test]
fn add_event_listener_returns_the_ids_in_call_order() {
    let (engine, _host) = engine_with_bindings();

    let ids: Vec<u32> = engine
        .eval(
            "const n = __inca_native__.createNode('div'); \
             [__inca_native__.addEventListener(n, 'a'), \
              __inca_native__.addEventListener(n, 'b', true), \
              __inca_native__.addEventListener(n, 'a')]",
        )
        .unwrap();

    assert_eq!(ids, [0, 1, 2]);
}

#[test]
fn add_event_listener_on_an_unknown_node_raises_type_error_and_takes_no_id() {
    let (engine, _host) = engine_with_bindings();

    engine
        .eval::<()>(
            "globalThis.n = __inca_native__.createNode('div'); \
             globalThis.typeError = false; \
             try { __inca_native__.addEventListener(9999, 'click'); } \
             catch (e) { globalThis.typeError = e instanceof TypeError; }",
        )
        .unwrap();

    assert!(engine.eval::<bool>("typeError").unwrap());
    assert_eq!(
        engine
            .eval::<u32>("__inca_native__.addEventListener(n, 'click')")
            .unwrap(),
        0
    );
}

#[test]
fn add_event_listener_raises_range_error_once_the_ids_run_out() {
    let (engine, host) = engine_with_bindings();
    host.borrow_mut().listeners.next_callback = u32::MAX;

    let range_error: bool = engine
        .eval(
            "(() => { const n = __inca_native__.createNode('div'); \
             try { __inca_native__.addEventListener(n, 'click'); return false; } \
             catch (e) { return e instanceof RangeError; } })()",
        )
        .unwrap();

    assert!(range_error);
    assert!(host.borrow().listeners.is_empty());
}

#[test]
fn unregister_reports_whether_it_removed_anything() {
    let mut listeners = EventListeners::default();
    listeners.register(1, "click", 7);

    assert!(listeners.unregister(1, "click", 7));
    assert!(listeners.callbacks_for(1, "click").is_empty());
    assert!(!listeners.unregister(1, "click", 7));
    assert!(!listeners.unregister(99, "click", 7));
}

#[test]
fn release_returns_every_callback_on_a_node() {
    let mut listeners = EventListeners::default();
    listeners.register(1, "click", 7);
    listeners.register(1, "focus", 8);
    listeners.register(2, "click", 9);

    let mut released = listeners.release(1);
    released.sort_unstable();

    assert_eq!(released, vec![7, 8]);
    assert!(listeners.callbacks_for(1, "click").is_empty());
    assert_eq!(
        listeners.callbacks_for(2, "click"),
        &[9],
        "another node's registrations must survive"
    );
}

#[test]
fn remove_event_listener_drops_the_registration() {
    let (engine, host) = engine_with_bindings();

    let removed: bool = engine
        .eval(
            r"
            const node = __inca_native__.createNode('div');
            const id = __inca_native__.addEventListener(node, 'click');
            __inca_native__.removeEventListener(node, 'click', id);
            ",
        )
        .unwrap();

    assert!(removed);
    assert!(host.borrow().listeners.callbacks_for(1, "click").is_empty());
}

#[test]
fn remove_event_listener_on_a_gone_node_is_not_an_error() {
    let (engine, _host) = engine_with_bindings();

    let removed: bool = engine
        .eval(
            r"
            const node = __inca_native__.createNode('div');
            __inca_native__.destroyNode(node);
            __inca_native__.removeEventListener(node, 'click', 3);
            ",
        )
        .unwrap();

    assert!(
        !removed,
        "removing after a destroy races normal teardown and must not throw"
    );
}
