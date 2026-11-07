// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The native record of an event: the structs, their lifetime marker and
//! their trace.

use std::cell::{Cell, RefCell};

#[allow(clippy::disallowed_types)]
use rquickjs::JsLifetime;
use rquickjs::class::{JsClass, Readable, Trace, Tracer};
use rquickjs::function::Constructor;
use rquickjs::{Ctx, Object, Result as JsResult};

use super::Flags;
use super::payload::Payload;

// A derive on a struct with a lifetime generates items that an `allow` on the struct does not
// cover, so the struct sits in a module of its own.
#[allow(clippy::disallowed_macros, clippy::disallowed_types)]
mod incaevent {
    use rquickjs::JsLifetime;

    use super::{Held, State};

    /// An event: the object every listener of one walk receives, and the state
    /// its getters and methods read.
    #[derive(JsLifetime)]
    pub(in crate::event) struct IncaEvent<'js> {
        pub(in crate::event) state: State,
        pub(in crate::event) held: Held<'js>,
    }
}

pub(super) use incaevent::IncaEvent;

// A derive on a struct with a lifetime generates items that an `allow` on the struct does not
// cover, so the struct sits in a module of its own.
#[allow(clippy::disallowed_macros, clippy::disallowed_types)]
mod held {
    use std::cell::RefCell;

    use rquickjs::{JsLifetime, Value};

    /// Every JS value an event holds. Each one is traced, so it lives as long as
    /// the event, and a cycle through it is collected. An untraced value aborts
    /// the process when the runtime drops.
    #[derive(JsLifetime)]
    pub(in crate::event) struct Held<'js> {
        /// What stands for the node the event was last dispatched at, `None`
        /// before the first dispatch.
        pub(in crate::event) target: RefCell<Option<Value<'js>>>,
        /// What stands for each node of the path while a walk runs, target
        /// first. Empty outside a walk.
        pub(in crate::event) path: RefCell<Vec<Value<'js>>>,
        /// What stands for the other node of a mouse or focus event.
        pub(in crate::event) related: Option<Value<'js>>,
        /// The `detail` of a `CustomEvent`, `None` for any other event.
        pub(in crate::event) detail: RefCell<Option<Value<'js>>>,
    }
}

pub(super) use held::Held;

/// The fields of an [`IncaEvent`] that hold no JS value. It has no lifetime
/// parameter, so a JS value cannot be added here. A walk sets the cells
/// between listener calls. `initEvent` sets the type and the flags of an idle
/// event.
#[allow(clippy::disallowed_macros)]
#[derive(JsLifetime)]
pub(super) struct State {
    pub(super) name: RefCell<String>,
    pub(super) flags: Cell<Flags>,
    pub(super) payload: Payload,
    pub(super) time_stamp: f64,
    pub(super) trusted: Cell<bool>,
    /// The number of the last walk that reached a listener.
    pub(super) event_id: Cell<Option<u64>>,
    pub(super) dispatching: Cell<bool>,
    pub(super) stop: Cell<bool>,
    pub(super) immediate: Cell<bool>,
    pub(super) canceled: Cell<bool>,
    /// Whether the running listener is passive.
    pub(super) passive: Cell<bool>,
    pub(super) phase: Cell<u8>,
    /// The index in the path of the node whose listeners run.
    pub(super) current: Cell<Option<usize>>,
}

impl<'js> Trace<'js> for IncaEvent<'js> {
    fn trace<'a>(&self, tracer: Tracer<'a, 'js>) {
        let IncaEvent { state: _, held } = self;
        held.trace(tracer);
    }
}

// Runs inside a GC callback of QuickJS: it must not panic, so no `borrow()`,
// `unwrap` or indexing here.
impl<'js> Trace<'js> for Held<'js> {
    fn trace<'a>(&self, tracer: Tracer<'a, 'js>) {
        // No `..` here: a new field must fail to compile until it is traced.
        let Held {
            target,
            path,
            related,
            detail,
        } = self;
        related.trace(tracer);
        // rquickjs traces a `Readable` class even mid-walk, so each cell guards itself.
        if let Ok(target) = target.try_borrow() {
            target.trace(tracer);
        }
        if let Ok(path) = path.try_borrow() {
            path.trace(tracer);
        }
        if let Ok(detail) = detail.try_borrow() {
            detail.trace(tracer);
        }
    }
}

impl<'js> JsClass<'js> for IncaEvent<'js> {
    const NAME: &'static str = "Event";

    type Mutable = Readable;

    // An event takes the prototype of its kind from the vault when it is made.
    fn prototype(_ctx: &Ctx<'js>) -> JsResult<Option<Object<'js>>> {
        Ok(None)
    }

    fn constructor(_ctx: &Ctx<'js>) -> JsResult<Option<Constructor<'js>>> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::test_support::engine_with_bindings;
    use crate::tests::{
        at, dispatcher_with_engine, eval_json, fire_and_read, key_payload, listen, mouse_at,
        pointer_click, read_out, tree_of_three, wheel_at,
    };
    use gpui::TestAppContext;
    use inca_gpui::{EventPayload, MousePayload};
    use inca_test_macros::gpui_cases;
    use rquickjs::Class;
    use rquickjs::Value;
    use rstest::rstest;

    #[test]
    fn a_collection_with_the_cells_borrowed_does_not_abort() {
        let (engine, _host) = engine_with_bindings();
        engine
            .eval::<()>(
                r"
                globalThis.detail = {};
                globalThis.kept = new CustomEvent('x', { detail });
                ",
            )
            .unwrap();

        engine.with(|ctx| {
            let this: Value = ctx.globals().get("kept").unwrap();
            let event = Class::<IncaEvent>::from_value(&this).unwrap();
            let event = event.borrow();
            let _target = event.held.target.borrow_mut();
            let _path = event.held.path.borrow_mut();
            let _detail = event.held.detail.borrow_mut();
            ctx.run_gc();
        });

        assert!(engine.eval::<bool>("kept.detail === detail").unwrap());
    }

    fn mouse_with_button(button: u8) -> EventPayload {
        EventPayload::Mouse(MousePayload {
            button,
            ..MousePayload::at(at(1.0, 1.0), gpui::Modifiers::none())
        })
    }

    fn mouse_button(button: u8) -> EventPayload {
        EventPayload::Mouse(MousePayload {
            button,
            buttons: inca_gpui::buttons_mask(button),
            ..MousePayload::at(at(1.0, 1.0), gpui::Modifiers::none())
        })
    }

    #[gpui::test]
    fn a_host_event_owns_only_is_trusted(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "click",
            "(e) => out.push(Reflect.ownKeys(e))",
        );

        let out = fire_and_read(cx, &dispatcher, node, "click", &pointer_click());

        assert_eq!(out, serde_json::json!([["isTrusted"]]));
    }

    #[test]
    fn a_custom_event_owns_only_is_trusted() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            "Reflect.ownKeys(new CustomEvent('x', { detail: 1 }))",
        );

        assert_eq!(got, serde_json::json!(["isTrusted"]));
    }

    #[test]
    fn the_is_trusted_descriptor_reads_enumerable_only() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const d = Object.getOwnPropertyDescriptor(new Event('x'), 'isTrusted');
                return [typeof d.get, typeof d.set, d.enumerable, d.configurable];
            })()
            ",
        );

        assert_eq!(
            got,
            serde_json::json!(["function", "undefined", true, false])
        );
    }

    #[gpui::test]
    fn every_event_shares_one_is_trusted_getter(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(&dispatcher, node, "click", "(e) => (globalThis.kept = e)");
        fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);

        let shared = dispatcher
            .engine
            .eval::<bool>(
                r"
                const getterOf = (e) => Object.getOwnPropertyDescriptor(e, 'isTrusted').get;
                getterOf(kept) === getterOf(new Event('x'));
                ",
            )
            .unwrap();

        assert!(shared);
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::mouse("mousedown", mouse_at(1.0, 1.0), "clientX")]
    #[case::mouse_related_target("mouseover", mouse_at(1.0, 1.0), "relatedTarget")]
    #[case::pointer("click", pointer_click(), "pointerId")]
    #[case::wheel("wheel", wheel_at(1.0, 1.0), "deltaX")]
    #[case::keyboard("keydown", key_payload(gpui::Modifiers::none()), "key")]
    #[case::focus("focus", EventPayload::Focus { related_target: None }, "relatedTarget")]
    fn a_field_is_a_getter_on_the_prototype_of_its_kind(
        cx: &mut TestAppContext,
        event: &str,
        payload: EventPayload,
        key: &str,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            event,
            &r"
            (e) => {
                const d = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(e), 'KEY');
                out.push([typeof d.get, typeof d.set, d.enumerable, d.configurable]);
            }
            "
            .replace("KEY", key),
        );

        let out = fire_and_read(cx, &dispatcher, node, event, &payload);

        assert_eq!(
            out,
            serde_json::json!([["function", "undefined", true, true]])
        );
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::mouse(
        "mousedown",
        mouse_at(1.0, 1.0),
        "Object.hasOwn(parent, 'which') && Object.getPrototypeOf(parent) === Event.prototype"
    )]
    #[case::pointer("click", pointer_click(), "Object.hasOwn(parent, 'clientX')")]
    #[case::wheel("wheel", wheel_at(1.0, 1.0), "Object.hasOwn(parent, 'clientX')")]
    #[case::keyboard(
        "keydown",
        key_payload(gpui::Modifiers::none()),
        "Object.hasOwn(parent, 'which') && Object.getPrototypeOf(parent) === Event.prototype"
    )]
    #[case::focus(
        "focus",
        EventPayload::Focus { related_target: None },
        "Object.hasOwn(parent, 'which') && Object.getPrototypeOf(parent) === Event.prototype"
    )]
    fn the_prototype_of_a_kind_inherits_from_its_parent(
        cx: &mut TestAppContext,
        event: &str,
        payload: EventPayload,
        check: &str,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            event,
            &r"
            (e) => {
                const parent = Object.getPrototypeOf(Object.getPrototypeOf(e));
                out.push(CHECK);
            }
            "
            .replace("CHECK", check),
        );

        let out = fire_and_read(cx, &dispatcher, node, event, &payload);

        assert_eq!(out, serde_json::json!([true]));
    }

    /// One body: the events of five kinds are fired once and read across.
    #[gpui::test]
    fn a_field_getter_throws_type_error_for_an_event_of_another_kind(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        for event in ["mousedown", "click", "wheel", "keydown", "focus"] {
            listen(
                &dispatcher,
                node,
                event,
                "(e) => ((globalThis.kept ??= {})[e.type] = e)",
            );
        }
        fire_and_read(cx, &dispatcher, node, "mousedown", &mouse_at(1.0, 1.0));
        fire_and_read(cx, &dispatcher, node, "click", &pointer_click());
        fire_and_read(cx, &dispatcher, node, "wheel", &wheel_at(1.0, 1.0));
        fire_and_read(
            cx,
            &dispatcher,
            node,
            "keydown",
            &key_payload(gpui::Modifiers::none()),
        );
        let focus = EventPayload::Focus {
            related_target: None,
        };
        fire_and_read(cx, &dispatcher, node, "focus", &focus);

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const getterOf = (event, key) => {
                    let owner = event;
                    while (!Object.hasOwn(owner, key)) {
                        owner = Object.getPrototypeOf(owner);
                    }
                    return Object.getOwnPropertyDescriptor(owner, key).get;
                };
                const read = (event, key, receiver) => {
                    try {
                        return 'ran ' + getterOf(event, key).call(receiver);
                    } catch (err) {
                        return err instanceof TypeError;
                    }
                };
                return [
                    read(kept.mousedown, 'clientX', kept.keydown),
                    read(kept.click, 'pointerId', kept.mousedown),
                    read(kept.wheel, 'deltaX', kept.mousedown),
                    read(kept.keydown, 'key', kept.mousedown),
                    read(kept.focus, 'relatedTarget', kept.mousedown),
                    read(kept.mousedown, 'relatedTarget', kept.focus),
                    read(kept.keydown, 'which', new Event('x')),
                    read(kept.focus, 'relatedTarget', new UIEvent('x')),
                    read(new CustomEvent('x'), 'detail', new Event('x'))
                ];
            })()
            ",
        );

        assert_eq!(
            got,
            serde_json::json!([true, true, true, true, true, true, true, true, true])
        );
    }

    #[gpui::test]
    fn a_mouse_field_getter_reads_a_wheel_event(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        for event in ["mousedown", "wheel"] {
            listen(
                &dispatcher,
                node,
                event,
                "(e) => ((globalThis.kept ??= {})[e.type] = e)",
            );
        }
        fire_and_read(cx, &dispatcher, node, "mousedown", &mouse_at(1.0, 1.0));
        fire_and_read(cx, &dispatcher, node, "wheel", &wheel_at(2.0, 1.0));

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const mouse = Object.getPrototypeOf(kept.mousedown);
                return Object.getOwnPropertyDescriptor(mouse, 'clientX').get.call(kept.wheel);
            })()
            ",
        );

        assert_eq!(got, serde_json::json!(2));
    }

    #[rstest]
    #[case::event("Event")]
    #[case::custom_event("CustomEvent")]
    fn a_constructor_without_an_object_prototype_gives_the_kind_s_prototype(
        #[case] constructor: &str,
    ) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                (() => {{
                    function target() {{}}
                    target.prototype = null;
                    const e = Reflect.construct({constructor}, ['x'], target);
                    return Object.getPrototypeOf(e) === {constructor}.prototype;
                }})()
                "
            ),
        );

        assert_eq!(got, serde_json::json!(true));
    }

    #[gpui::test]
    fn the_targets_hook_receives_its_ids_past_an_inherited_index_setter(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, parent, child] = tree_of_three(&host);
        listen(&dispatcher, child, "click", "() => {}");
        dispatcher
            .engine
            .eval::<()>(
                r"
                Object.defineProperty(Array.prototype, 0, {
                    set(value) {
                        globalThis.caught = value;
                    },
                    configurable: true,
                });
                globalThis.__inca_targets__ = (ids) => {
                    globalThis.ids = ids;
                    return ids.map((id) => id);
                };
                ",
            )
            .unwrap();
        // Not `fire_and_read`: its `JSON.stringify` would meet the setter.
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            dispatcher.fire(child, "click", &EventPayload::None, window, cx);
        });

        let got = eval_json(&dispatcher, "[ids, globalThis.caught === undefined]");

        assert_eq!(got, serde_json::json!([[child, parent, outer], true]));
    }

    #[gpui::test]
    fn composed_path_entries_ignore_an_inherited_index_setter(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, parent, child] = tree_of_three(&host);
        listen(
            &dispatcher,
            parent,
            "click",
            "(e) => (globalThis.path = e.composedPath())",
        );
        dispatcher
            .engine
            .eval::<()>(
                r"
                Object.defineProperty(Array.prototype, 0, {
                    set(value) {
                        globalThis.caught = value;
                    },
                    configurable: true,
                });
                ",
            )
            .unwrap();
        // Not `fire_and_read`: its `JSON.stringify` would meet the setter.
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            dispatcher.fire(child, "click", &EventPayload::None, window, cx);
        });

        let got = eval_json(&dispatcher, "[path, globalThis.caught === undefined]");

        assert_eq!(got, serde_json::json!([[child, parent, outer], true]));
    }

    #[test]
    fn the_event_id_descriptor_reads_configurable_only() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const d = Object.getOwnPropertyDescriptor(Event.prototype, 'eventId');
                return [typeof d.get, typeof d.set, d.enumerable, d.configurable];
            })()
            ",
        );

        assert_eq!(
            got,
            serde_json::json!(["function", "undefined", false, true])
        );
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::key_code("keyCode", serde_json::json!(65))]
    #[case::which("which", serde_json::json!(65))]
    #[case::char_code("charCode", serde_json::json!(0))]
    #[case::code("code", serde_json::json!(""))]
    fn a_host_key_event_reads_each_legacy_field(
        cx: &mut TestAppContext,
        key: &str,
        want: serde_json::Value,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "keydown",
            &format!("(e) => out.push(e.{key})"),
        );

        let out = fire_and_read(
            cx,
            &dispatcher,
            node,
            "keydown",
            &key_payload(gpui::Modifiers::none()),
        );

        assert_eq!(out, serde_json::json!([want]));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::letter("a", 97)]
    #[case::above_the_bmp("😀", 55_357)]
    fn a_host_keypress_reads_the_character_in_each_legacy_field(
        cx: &mut TestAppContext,
        key: &str,
        want: u32,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "keypress",
            "(e) => out.push([e.charCode, e.keyCode, e.which])",
        );
        let payload = EventPayload::Key(inca_gpui::KeyPayload {
            key: key.into(),
            repeat: false,
            modifiers: gpui::Modifiers::none(),
            is_composing: false,
        });

        let out = fire_and_read(cx, &dispatcher, node, "keypress", &payload);

        assert_eq!(out, serde_json::json!([[want, want, want]]), "{key}");
    }

    #[gpui::test]
    fn the_view_of_a_host_event_reads_null(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "mousedown",
            "(e) => out.push(e.view === null)",
        );

        let out = fire_and_read(cx, &dispatcher, node, "mousedown", &mouse_at(1.0, 1.0));

        assert_eq!(out, serde_json::json!([true]));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::button_event("mouseup", mouse_button(2), 3)]
    #[case::pointer_click("click", pointer_click(), 1)]
    #[case::dblclick("dblclick", mouse_with_button(2), 3)]
    #[case::auxclick("auxclick", mouse_with_button(2), 3)]
    #[case::contextmenu("contextmenu", mouse_with_button(2), 3)]
    #[case::focus("focus", EventPayload::Focus { related_target: None }, 0)]
    fn the_which_of_a_host_event_reads_its_button_or_zero(
        cx: &mut TestAppContext,
        event: &str,
        payload: EventPayload,
        want: u32,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(&dispatcher, node, event, "(e) => out.push(e.which)");

        let out = fire_and_read(cx, &dispatcher, node, event, &payload);

        assert_eq!(out, serde_json::json!([want]));
    }

    /// One body: the presses set the held-button tracker that the move reads.
    #[gpui_cases]
    #[gpui::test]
    #[case::right(&[2], 3)]
    #[case::middle_before_right(&[2, 1], 2)]
    fn a_move_names_its_lowest_held_button_in_which(
        cx: &mut TestAppContext,
        pressed: &[u8],
        want: u32,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(&dispatcher, node, "mousemove", "(e) => out.push(e.which)");
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            for &button in pressed {
                dispatcher.update_held_buttons("mousedown", button);
            }
            dispatcher.fire(node, "mousemove", &mouse_at(1.0, 1.0), window, cx);
        });

        assert_eq!(read_out(&dispatcher), serde_json::json!([want]));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::keyboard("keydown", key_payload(gpui::Modifiers::none()))]
    #[case::focus("focus", EventPayload::Focus { related_target: None })]
    fn the_detail_of_a_host_key_or_focus_event_reads_zero(
        cx: &mut TestAppContext,
        event: &str,
        payload: EventPayload,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(&dispatcher, node, event, "(e) => out.push(e.detail)");

        let out = fire_and_read(cx, &dispatcher, node, event, &payload);

        assert_eq!(out, serde_json::json!([0]));
    }

    /// One body: the presses set the held-button tracker that the next press and the wheel read.
    #[gpui_cases]
    #[gpui::test]
    #[case::mousedown("mousedown", &[1], mouse_button(2), 3)]
    #[case::wheel("wheel", &[2], wheel_at(1.0, 1.0), 0)]
    fn which_of_a_press_or_a_wheel_skips_the_held_buttons(
        cx: &mut TestAppContext,
        event: &str,
        pressed: &[u8],
        payload: EventPayload,
        want: u32,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(&dispatcher, node, event, "(e) => out.push(e.which)");
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            for &button in pressed {
                dispatcher.update_held_buttons("mousedown", button);
            }
            dispatcher.fire(node, event, &payload, window, cx);
        });

        let out = read_out(&dispatcher);

        assert_eq!(
            out.as_array().unwrap().last().unwrap(),
            &serde_json::json!(want)
        );
    }

    #[rstest]
    #[case::input_data("new InputEvent('x')", "data", "new MouseEvent('x')")]
    #[case::composition_data("new CompositionEvent('x')", "data", "new KeyboardEvent('x')")]
    #[case::get_target_ranges("new InputEvent('x')", "getTargetRanges", "new MouseEvent('x')")]
    fn an_input_or_composition_field_throws_on_another_kind(
        #[case] owner: &str,
        #[case] key: &str,
        #[case] receiver: &str,
    ) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                (() => {{
                    let proto = {owner};
                    while (!Object.hasOwn(proto, '{key}')) {{
                        proto = Object.getPrototypeOf(proto);
                    }}
                    const descriptor = Object.getOwnPropertyDescriptor(proto, '{key}');
                    const getter = descriptor.get ?? descriptor.value;
                    try {{
                        return 'ran ' + getter.call({receiver});
                    }} catch (err) {{
                        return err instanceof TypeError;
                    }}
                }})()
                "
            ),
        );

        assert_eq!(got, serde_json::json!(true));
    }
}
