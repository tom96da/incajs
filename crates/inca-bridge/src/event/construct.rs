// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;

use inca_gpui::{EventPayload, NodeId};
use inca_jsenv::class;
use rquickjs::convert::Coerced;
use rquickjs::function::{IntoJsFunc, Opt, ParamRequirement, Params, This};
use rquickjs::{Class, Ctx, Exception, Function, Object, Result as JsResult, Value};

use super::members::PHASES;
use super::payload::{DELTA_MODES, KEY_LOCATIONS, Payload, Ui};
use super::walk::Walk;
use super::{
    COMPOSITION, CUSTOM, EVENT, FOCUS, Flags, Held, INPUT, IncaEvent, KEYBOARD, MOUSE, POINTER,
    State, UI, WHEEL, child_of, constants, field, given, initialize, new_event, other_kind,
    targets_of, with_event,
};
use crate::EventTargetId;
use crate::bindings::convert::or_null;
use init::{Init, input, keyboard, mouse, pointer, wheel};

mod init;

/// Each class of event: the kind it builds, its name, and the kind whose constructor its
/// constructor inherits from.
pub(super) const CLASSES: [(&str, &str, Option<&str>); 10] = [
    (EVENT, "Event", None),
    (CUSTOM, "CustomEvent", Some(EVENT)),
    (UI, "UIEvent", Some(EVENT)),
    (FOCUS, "FocusEvent", Some(UI)),
    (MOUSE, "MouseEvent", Some(UI)),
    (POINTER, "PointerEvent", Some(MOUSE)),
    (WHEEL, "WheelEvent", Some(MOUSE)),
    (KEYBOARD, "KeyboardEvent", Some(UI)),
    (INPUT, "InputEvent", Some(UI)),
    (COMPOSITION, "CompositionEvent", Some(UI)),
];

/// A new event for a walk, ready to deliver. `targets` holds what stands for each target of the
/// path, target first.
pub(super) fn create<'js>(
    ctx: &Ctx<'js>,
    walk: &Walk<'_>,
    id: u64,
    targets: Vec<Value<'js>>,
) -> JsResult<Class<'js, IncaEvent<'js>>> {
    let related = match related_of(walk.payload) {
        Some(node) => targets_of(ctx, &[EventTargetId::Node(node)])?.pop(),
        None => None,
    };
    let payload = Payload::from_host(walk.name, walk.payload);
    let kind = Payload::host_kind(walk.payload);
    let state = State::new(
        walk.name.to_owned(),
        walk.flags,
        payload,
        walk.trusted,
        inca_jsenv::now(ctx)?,
    );
    state.event_id.set(Some(id));
    state.dispatching.set(true);
    let event = IncaEvent {
        state,
        held: Held {
            target: RefCell::new(targets.first().cloned()),
            path: RefCell::new(targets),
            related,
            detail: RefCell::new(None),
        },
    };
    new_event(ctx, kind, event)
}

/// The other node `payload` names, if any.
fn related_of(payload: &EventPayload) -> Option<NodeId> {
    match payload {
        EventPayload::Mouse(mouse) => mouse.related_target,
        EventPayload::Wheel(wheel) => wheel.mouse.related_target,
        EventPayload::Focus { related_target } => *related_target,
        EventPayload::None
        | EventPayload::Key(_)
        | EventPayload::Input(_)
        | EventPayload::Composition { .. } => None,
    }
}

/// `CustomEvent.prototype`: the `detail` getter and `initCustomEvent`. It inherits from
/// `event`.
pub(super) fn custom_prototype<'js>(ctx: &Ctx<'js>, event: &Object<'js>) -> JsResult<Object<'js>> {
    let proto = child_of(ctx, event)?;
    field(ctx, &proto, "detail", |ctx, event| {
        event
            .held
            .detail
            .borrow()
            .clone()
            .ok_or_else(|| other_kind(ctx, "detail"))
    })?;
    let init = Function::new(
        ctx.clone(),
        |ctx: Ctx<'js>,
         This(this): This<Value<'js>>,
         event_type: Coerced<String>,
         bubbles: Opt<Coerced<bool>>,
         cancelable: Opt<Coerced<bool>>,
         detail: Opt<Value<'js>>| {
            with_event(&ctx, &this, |event| {
                if event.held.detail.borrow().is_none() {
                    return Err(other_kind(&ctx, "initCustomEvent"));
                }
                if initialize(event, event_type.0, given(&bubbles), given(&cancelable)) {
                    let detail = or_null(&ctx, detail.0.filter(|detail| !detail.is_undefined()))?;
                    drop(event.held.detail.replace(Some(detail)));
                }
                Ok(())
            })
        },
    )?;
    class::define_method(&proto, "initCustomEvent", init)?;
    Ok(proto)
}

/// Installs a global constructor for each of [`CLASSES`], linked with the prototype of the same
/// index in `prototypes`, and the constants of `Event`, `WheelEvent` and `KeyboardEvent`.
pub(super) fn constructors<'js>(ctx: &Ctx<'js>, prototypes: &[Object<'js>; 10]) -> JsResult<()> {
    let globals = ctx.globals();
    let mut made: Vec<(&str, Function<'js>)> = Vec::new();
    for (&(kind, name, parent), proto) in CLASSES.iter().zip(prototypes) {
        let function = class::define_class(ctx, proto, name, 1, Construct { name, kind })?;
        if let Some((_, parent)) = made.iter().find(|(made, _)| Some(*made) == parent) {
            function.set_prototype(Some(parent))?;
        }
        globals.set(name, function.clone())?;
        made.push((kind, function));
    }
    for (kind, function) in &made {
        match *kind {
            EVENT => constants(function, &PHASES)?,
            WHEEL => constants(function, &DELTA_MODES)?,
            KEYBOARD => constants(function, &KEY_LOCATIONS)?,
            _ => {}
        }
    }
    Ok(())
}

/// The body of each event constructor.
struct Construct {
    name: &'static str,
    kind: &'static str,
}

impl<'js> IntoJsFunc<'js, ()> for Construct {
    fn param_requirements() -> ParamRequirement {
        ParamRequirement::any()
    }

    fn call<'a>(&self, params: Params<'a, 'js>) -> JsResult<Value<'js>> {
        let ctx = params.ctx();
        let name = self.name;
        let type_error = |message: &str| {
            Exception::throw_type(ctx, &format!("cannot construct {name}: {message}"))
        };
        if !params.is_constructor() {
            return Err(type_error(&format!(
                "the event constructor needs new, so write new {name}(type)"
            )));
        }
        let Some(event_type) = params.arg(0) else {
            return Err(type_error(concat!(
                "the event type is missing, so pass a type string such as \"click\" ",
                "as the first argument"
            )));
        };
        let event_type = event_type.get::<Coerced<String>>()?.0;
        let object = match params.arg(1) {
            Some(value) if !value.is_undefined() && !value.is_null() => {
                Some(value.into_object().ok_or_else(|| {
                    type_error(concat!(
                        "the event init is not an object, so pass an object such as ",
                        "{ bubbles: true } or leave it out"
                    ))
                })?)
            }
            _ => None,
        };
        let init = Init { ctx, name, object };
        let flags = Flags {
            bubbles: init.boolean("bubbles")?,
            cancelable: init.boolean("cancelable")?,
            composed: init.boolean("composed")?,
        };
        let (payload, related, detail) = self.payload(&init)?;
        let state = State::new(event_type, flags, payload, false, inca_jsenv::now(ctx)?);
        let event = IncaEvent {
            state,
            held: Held {
                target: RefCell::new(None),
                path: RefCell::new(Vec::new()),
                related,
                detail: RefCell::new(detail),
            },
        };
        let event = new_event(ctx, self.kind, event)?;
        if let Some(default) = event.get_prototype() {
            event.set_prototype(Some(&class::new_target_prototype(&params, default)?))?;
        }
        Ok(event.into_value())
    }
}

/// What the init of one class gives: the payload, `relatedTarget` and `detail`.
type Built<'js> = (Payload, Option<Value<'js>>, Option<Value<'js>>);

impl Construct {
    /// The payload, `relatedTarget` and `detail` of an event of this class built from `init`.
    fn payload<'js>(&self, init: &Init<'_, 'js>) -> JsResult<Built<'js>> {
        match self.kind {
            EVENT => Ok((Payload::None, None, None)),
            CUSTOM => {
                let detail = or_null(init.ctx, init.member("detail")?)?;
                Ok((Payload::None, None, Some(detail)))
            }
            kind => {
                let ui = Ui {
                    detail: init.long("detail")?,
                    which: 0,
                };
                match kind {
                    UI => Ok((Payload::Ui(ui), None, None)),
                    FOCUS => {
                        let related = init.target("relatedTarget")?;
                        Ok((Payload::Focus(ui), related, None))
                    }
                    KEYBOARD => Ok((keyboard(init, ui)?, None, None)),
                    INPUT => Ok((input(init, ui)?, None, None)),
                    COMPOSITION => Ok((Payload::Composition(ui, init.string("data")?), None, None)),
                    _ => {
                        let (mouse, related) = mouse(init, ui)?;
                        let payload = match kind {
                            POINTER => Payload::Pointer(mouse, pointer(init)?),
                            WHEEL => Payload::Wheel(mouse, wheel(init)?),
                            _ => Payload::Mouse(mouse),
                        };
                        Ok((payload, related, None))
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::tests::{
        dispatcher_with_engine, eval_json, fire_and_read, key_payload, listen, mouse_at,
        pointer_click, wheel_at,
    };
    use gpui::TestAppContext;
    use inca_gpui::EventPayload;
    use inca_test_macros::gpui_cases;
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
    fn a_custom_event_carries_its_detail() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const d = { n: 1 };
                const a = new CustomEvent('a', { detail: d, bubbles: true });
                const b = new CustomEvent('b');
                const c = new CustomEvent('c', { detail: undefined });
                return [a.detail === d, a.bubbles, b.detail, c.detail];
            })()
            ",
        );
        assert_eq!(got, serde_json::json!([true, true, null, null]));
    }

    #[test]
    fn a_custom_event_inherits_from_event() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const a = new CustomEvent('a');
                return [
                    a instanceof CustomEvent,
                    a instanceof Event,
                    Object.getPrototypeOf(CustomEvent) === Event,
                    a.preventDefault === new Event('x').preventDefault,
                    CustomEvent.name,
                    Event.name,
                    Event.AT_TARGET,
                    CustomEvent.BUBBLING_PHASE,
                ];
            })()
            ",
        );
        assert_eq!(
            got,
            serde_json::json!([true, true, true, true, "CustomEvent", "Event", 2, 3])
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

    #[rstest]
    #[case::ui_event("UIEvent", "Event")]
    #[case::focus_event("FocusEvent", "UIEvent")]
    #[case::mouse_event("MouseEvent", "UIEvent")]
    #[case::pointer_event("PointerEvent", "MouseEvent")]
    #[case::wheel_event("WheelEvent", "MouseEvent")]
    #[case::keyboard_event("KeyboardEvent", "UIEvent")]
    #[case::input_event("InputEvent", "UIEvent")]
    #[case::composition_event("CompositionEvent", "UIEvent")]
    fn an_event_constructor_inherits_from_its_parent(#[case] name: &str, #[case] parent: &str) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            &format!(
                concat!(
                    "[Object.getPrototypeOf({name}) === {parent}, ",
                    "Object.getPrototypeOf({name}.prototype) === {parent}.prototype]"
                ),
                name = name,
                parent = parent,
            ),
        );

        assert_eq!(got, serde_json::json!([true, true]));
    }

    fn input_payload() -> EventPayload {
        EventPayload::Input(inca_gpui::InputPayload {
            data: None,
            input_type: inca_gpui::InputType::InsertText,
            is_composing: false,
        })
    }

    fn composition_payload() -> EventPayload {
        EventPayload::Composition {
            data: String::new(),
        }
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::mouse("mousedown", mouse_at(1.0, 1.0), "MouseEvent")]
    #[case::pointer("click", pointer_click(), "PointerEvent")]
    #[case::wheel("wheel", wheel_at(1.0, 1.0), "WheelEvent")]
    #[case::keyboard("keydown", key_payload(gpui::Modifiers::none()), "KeyboardEvent")]
    #[case::focus("focus", EventPayload::Focus { related_target: None }, "FocusEvent")]
    #[case::input("input", input_payload(), "InputEvent")]
    #[case::composition("compositionend", composition_payload(), "CompositionEvent")]
    fn a_host_event_reports_its_class_as_constructor(
        cx: &mut TestAppContext,
        event: &str,
        payload: EventPayload,
        class: &str,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            event,
            &format!("(e) => out.push(e.constructor === {class})"),
        );

        let out = fire_and_read(cx, &dispatcher, node, event, &payload);

        assert_eq!(out, serde_json::json!([true]));
    }

    #[rstest]
    #[case::wheel_constructor("WheelEvent.DOM_DELTA_PAGE", 2)]
    #[case::wheel_prototype("WheelEvent.prototype.DOM_DELTA_LINE", 1)]
    #[case::keyboard_constructor("KeyboardEvent.DOM_KEY_LOCATION_NUMPAD", 3)]
    #[case::keyboard_prototype("KeyboardEvent.prototype.DOM_KEY_LOCATION_RIGHT", 2)]
    fn an_event_constant_reads_its_value(#[case] constant: &str, #[case] want: i32) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        assert_eq!(eval_json(&dispatcher, constant), serde_json::json!(want));
    }

    #[rstest]
    #[case::focus("FocusEvent", "focus")]
    #[case::mouse("MouseEvent", "click")]
    fn a_built_event_keeps_its_related_target(#[case] class: &str, #[case] event_type: &str) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                (() => {{
                    const other = {{}};
                    return new {class}('{event_type}', {{ relatedTarget: other }}).relatedTarget === other;
                }})()
                "
            ),
        );

        assert_eq!(got, serde_json::json!(true));
    }

    #[test]
    fn init_custom_event_sets_the_detail() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const d = { n: 1 };
                const e = new CustomEvent('a', { detail: 2 });
                e.initCustomEvent('b', true, false, d);
                return [e.type, e.bubbles, e.cancelable, e.detail === d];
            })()
            ",
        );

        assert_eq!(got, serde_json::json!(["b", true, false, true]));
    }

    #[test]
    fn init_custom_event_reads_an_undefined_detail_as_null() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const e = new CustomEvent('a', { detail: 2 });
                e.initCustomEvent('b', false, false, undefined);
                return e.detail;
            })()
            ",
        );

        assert_eq!(got, serde_json::json!(null));
    }

    #[test]
    fn init_custom_event_throws_on_an_event_without_detail() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                try {
                    CustomEvent.prototype.initCustomEvent.call(new Event('a'), 'b');
                    return 'ran';
                } catch (err) {
                    return err instanceof TypeError;
                }
            })()
            ",
        );

        assert_eq!(got, serde_json::json!(true));
    }

    #[test]
    fn init_custom_event_during_its_dispatch_keeps_the_detail() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "x",
            r"
            (e) => {
                e.initCustomEvent('y', true, true, 9);
                out.push(e.detail);
            }
            ",
        );

        let got = eval_json(
            &dispatcher,
            &format!(
                "(__inca_native__.dispatchEvent({node}, new CustomEvent('x', {{ detail: 1 }})), out)"
            ),
        );

        assert_eq!(got, serde_json::json!([1]));
    }

    #[rstest]
    #[case::init_event("Event.prototype.initEvent")]
    #[case::init_custom_event("CustomEvent.prototype.initCustomEvent")]
    fn an_init_method_length_reads_one(#[case] method: &str) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        assert_eq!(
            eval_json(&dispatcher, &format!("{method}.length")),
            serde_json::json!(1)
        );
    }
}
