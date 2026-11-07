// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The event object a listener receives and the walk of one event over its
//! path.
//!
//! One event is one object of the native class [`IncaEvent`]. Its fields live
//! in Rust, and the getters and methods that read them sit on a prototype of
//! the event's kind, built once per realm. `isTrusted` is the one own property
//! of an event. The walk creates the event, runs the capture listeners from
//! `window` down to the target and then the bubble listeners from the target
//! up, all with that same object, then resets its `currentTarget` and
//! `eventPhase`. The global constructors, `Event`, `MouseEvent` and the other
//! kinds, build events of the same class, so a script can build an event and
//! dispatch it. The prototypes sit in the realm's vault, and Rust keeps only
//! their keys.
//!
//! The global `__inca_targets__` gives the objects that stand for the targets
//! of a walk: the nodes, then `document` and `window`.

use std::cell::{Cell, RefCell};

pub(crate) use inca_gpui::event_sink::Flags;
use inca_jsenv::{FunctionAccessor, vault};
use rquickjs::convert::Coerced;
use rquickjs::function::Opt;
use rquickjs::object::Property;
use rquickjs::{Array, Class, Ctx, Exception, IntoJs, Object, Result as JsResult, Value};

mod construct;
mod key_code;
mod members;
mod payload;
mod record;
mod walk;

use record::{Held, IncaEvent, State};
use walk::targets_of;
pub(crate) use walk::{Walk, click, dispatch_event, listened, lose_focus, walk};

/// The vault key of the event store: the prototype of each kind of event
/// under the kind, and the `isTrusted` getter.
const STORE: &str = "events";
/// The store key of the getter every event owns as `isTrusted`.
const IS_TRUSTED: &str = "isTrusted";
/// The kind of scroll events and of events built with `new Event`. A kind is
/// the store key of its prototype, `Event.prototype` for this one.
const EVENT: &str = "event";
/// The kind of events built with `new UIEvent` and of the prototype that the focus, mouse and
/// keyboard prototypes inherit from. Its prototype inherits from the [`EVENT`] one.
const UI: &str = "ui";
/// The kind of events built with `new CustomEvent`. Its prototype inherits
/// from the [`EVENT`] one.
const CUSTOM: &str = "custom";
/// The kind of focus events. Its prototype inherits from the [`UI`] one.
const FOCUS: &str = "focus";
/// The kind of mouse events with no pointer source. Its prototype inherits
/// from the [`UI`] one.
const MOUSE: &str = "mouse";
/// The kind of mouse events with a pointer source: `click`, `auxclick` and
/// `contextmenu`. Its prototype inherits from the [`MOUSE`] one.
const POINTER: &str = "pointer";
/// The kind of wheel events. Its prototype inherits from the [`MOUSE`] one.
const WHEEL: &str = "wheel";
/// The kind of key events. Its prototype inherits from the [`UI`] one.
const KEYBOARD: &str = "keyboard";
/// The kind of `beforeinput` and `input`. Its prototype inherits from the [`UI`] one.
const INPUT: &str = "input";
/// The kind of the composition events. Its prototype inherits from the [`UI`] one.
const COMPOSITION: &str = "composition";

impl State {
    /// The state of an idle event created at `time_stamp` milliseconds.
    fn new(
        name: String,
        flags: Flags,
        payload: payload::Payload,
        trusted: bool,
        time_stamp: f64,
    ) -> Self {
        Self {
            name: RefCell::new(name),
            flags: Cell::new(flags),
            payload,
            time_stamp,
            trusted: Cell::new(trusted),
            event_id: Cell::new(None),
            dispatching: Cell::new(false),
            stop: Cell::new(false),
            immediate: Cell::new(false),
            canceled: Cell::new(false),
            passive: Cell::new(false),
            phase: Cell::new(0),
            current: Cell::new(None),
        }
    }
}

impl<'js> IncaEvent<'js> {
    /// What stands for the node whose listeners run, `None` outside a walk.
    fn current_target(&self) -> Option<Value<'js>> {
        let index = self.state.current.get()?;
        self.held.path.borrow().get(index).cloned()
    }

    /// Whether a cancel request takes effect now: the event is cancelable and
    /// the running listener is active.
    fn cancels(&self) -> bool {
        self.state.flags.get().cancelable && !self.state.passive.get()
    }
}

/// Creates the prototypes of `ctx`'s realm in its vault and the global
/// constructors. The first call wins, and later calls in the same realm leave
/// it as it is.
///
/// # Errors
///
/// Returns an error if the realm lacks an intrinsic or has no vault.
pub(crate) fn install(ctx: &Ctx<'_>) -> JsResult<()> {
    let vault = vault(ctx)?;
    if vault.contains_key(STORE)? {
        return Ok(());
    }
    let store = Object::new(ctx.clone())?;
    store.set_prototype(None)?;
    let event = members::event_prototype(ctx)?;
    let ui = payload::ui_prototype(ctx, &event)?;
    let mouse = payload::mouse_prototype(ctx, &ui)?;
    // In the order of `construct::CLASSES`.
    let prototypes = [
        event.clone(),
        construct::custom_prototype(ctx, &event)?,
        ui.clone(),
        payload::focus_prototype(ctx, &ui)?,
        mouse.clone(),
        payload::pointer_prototype(ctx, &mouse)?,
        payload::wheel_prototype(ctx, &mouse)?,
        payload::keyboard_prototype(ctx, &ui)?,
        payload::input_prototype(ctx, &ui)?,
        payload::composition_prototype(ctx, &ui)?,
    ];
    construct::constructors(ctx, &prototypes)?;
    for ((kind, ..), proto) in construct::CLASSES.iter().zip(prototypes) {
        store.set(*kind, proto)?;
    }
    store.set(IS_TRUSTED, members::is_trusted(ctx)?)?;
    vault.set(STORE, store)
}

/// `event` as an object of the prototype of `kind`, with the shared
/// `isTrusted` getter as its own property.
fn new_event<'js>(
    ctx: &Ctx<'js>,
    kind: &str,
    event: IncaEvent<'js>,
) -> JsResult<Class<'js, IncaEvent<'js>>> {
    let store: Object = vault(ctx)?.get(STORE)?;
    let event = Class::instance_proto(event, store.get(kind)?)?;
    event.prop(
        IS_TRUSTED,
        FunctionAccessor::new_get(store.get(IS_TRUSTED)?).enumerable(),
    )?;
    Ok(event)
}

/// Runs `run` on the event `this` names.
///
/// # Errors
///
/// Throws a `TypeError` when `this` is not an event, and passes on what `run`
/// returns.
fn with_event<'js, T>(
    ctx: &Ctx<'js>,
    this: &Value<'js>,
    run: impl FnOnce(&IncaEvent<'js>) -> JsResult<T>,
) -> JsResult<T> {
    let event = Class::<IncaEvent>::from_value(this).map_err(|_| {
        Exception::throw_type(
            ctx,
            concat!(
                "not an event object, so call the method on an Event ",
                "such as the one a listener receives"
            ),
        )
    })?;
    run(&event.borrow())
}

/// A new object that inherits from `parent`.
fn child_of<'js>(ctx: &Ctx<'js>, parent: &Object<'js>) -> JsResult<Object<'js>> {
    let child = Object::new(ctx.clone())?;
    child.set_prototype(Some(parent))?;
    Ok(child)
}

/// Defines each of `list` on `object` as an enumerable, read-only, non-configurable constant.
fn constants(object: &Object<'_>, list: &[(&str, i32)]) -> JsResult<()> {
    for &(name, value) in list {
        object.prop(name, Property::from(value).enumerable())?;
    }
    Ok(())
}

/// An optional boolean argument: absent reads as `false`.
fn given(flag: &Opt<Coerced<bool>>) -> bool {
    flag.0.as_ref().is_some_and(|flag| flag.0)
}

/// Re-initializes `event` with `event_type` and the two flags, as `initEvent` does: no stop and
/// no cancel, untrusted, the same `composed`. Returns `false` and changes nothing while the
/// event is dispatched.
fn initialize(event: &IncaEvent<'_>, event_type: String, bubbles: bool, cancelable: bool) -> bool {
    let state = &event.state;
    if state.dispatching.get() {
        return false;
    }
    *state.name.borrow_mut() = event_type;
    let composed = state.flags.get().composed;
    state.flags.set(Flags {
        bubbles,
        cancelable,
        composed,
    });
    for flag in [&state.stop, &state.immediate, &state.canceled] {
        flag.set(false);
    }
    state.trusted.set(false);
    true
}

/// The `TypeError` that `member` of one kind of event throws for an event of
/// another kind.
fn other_kind(ctx: &Ctx<'_>, member: &str) -> rquickjs::Error {
    Exception::throw_type(
        ctx,
        &format!("{member} belongs to another kind of event, so use it on an event of that kind"),
    )
}

/// A new array of `items`. Each entry is defined, so no index setter on
/// `Array.prototype` or `Object.prototype` runs.
pub(crate) fn array_of<'js, T: IntoJs<'js>>(
    ctx: &Ctx<'js>,
    items: impl IntoIterator<Item = T>,
) -> JsResult<Array<'js>> {
    let array = Array::new(ctx.clone())?;
    for (index, item) in (0u32..).zip(items) {
        let value = item.into_js(ctx)?;
        array.as_object().prop(
            index,
            Property::from(value).writable().enumerable().configurable(),
        )?;
    }
    Ok(array)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use crate::tests::{
        RECORDER, dispatcher_on_clock, dispatcher_with_engine, eval_json, fire_and_read, listen,
        seen_rows, tree_of_three,
    };
    use gpui::TestAppContext;
    use inca_gpui::EventPayload;
    use inca_jsenv::Clock;

    /// A clock that `at` moves.
    fn test_clock() -> (Clock, Rc<Cell<Instant>>) {
        let at = Rc::new(Cell::new(Instant::now()));
        let clock: Clock = {
            let at = Rc::clone(&at);
            Rc::new(move || at.get())
        };
        (clock, at)
    }

    #[gpui::test]
    fn the_time_stamp_of_a_host_event_is_the_clock_reading_at_its_creation(
        cx: &mut TestAppContext,
    ) {
        let (clock, at) = test_clock();
        let (dispatcher, host) = dispatcher_on_clock(clock);
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        dispatcher.engine.eval::<()>(RECORDER).unwrap();
        listen(&dispatcher, node, "click", "record");
        at.set(at.get() + Duration::from_millis(40));

        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            dispatcher.fire(node, "click", &EventPayload::None, window, cx);
        });

        let stamp = seen_rows(&dispatcher)[0][9].as_f64().unwrap();
        // Float rounding of seconds to milliseconds.
        assert!((stamp - 40.0).abs() < 1e-9, "{stamp}");
    }

    #[test]
    fn the_time_stamp_of_a_new_event_is_the_clock_reading_at_its_creation() {
        let (clock, at) = test_clock();
        let (dispatcher, _host) = dispatcher_on_clock(clock);
        at.set(at.get() + Duration::from_millis(30));

        let got = eval_json(&dispatcher, "new Event('x').timeStamp");

        // Float rounding of seconds to milliseconds.
        assert!((got.as_f64().unwrap() - 30.0).abs() < 1e-9, "{got}");
    }

    #[test]
    fn init_event_sets_the_type_and_the_flags() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const e = new Event('a');
                e.initEvent(5, 1, 0);
                return [e.type, e.bubbles, e.cancelable];
            })()
            ",
        );

        assert_eq!(got, serde_json::json!(["5", true, false]));
    }

    #[test]
    fn init_event_defaults_the_flags_to_false() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const e = new Event('a', { bubbles: true, cancelable: true });
                e.initEvent('b');
                return [e.type, e.bubbles, e.cancelable];
            })()
            ",
        );

        assert_eq!(got, serde_json::json!(["b", false, false]));
    }

    #[test]
    fn init_event_without_a_type_throws_a_type_error() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                try {
                    new Event('a').initEvent();
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
    fn init_event_during_its_dispatch_changes_nothing() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "x",
            r"
            (e) => {
                e.initEvent('y', true, true);
                out.push([e.type, e.bubbles, e.cancelable]);
            }
            ",
        );

        let got = eval_json(
            &dispatcher,
            &format!("(__inca_native__.dispatchEvent({node}, new Event('x')), out)"),
        );

        assert_eq!(got, serde_json::json!([["x", false, false]]));
    }

    #[test]
    fn init_event_sets_the_type_and_bubbling_for_the_next_dispatch() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, _, child] = tree_of_three(&host);
        listen(&dispatcher, child, "b", "() => out.push('child')");
        listen(&dispatcher, outer, "b", "() => out.push('outer')");

        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                (() => {{
                    const e = new Event('a');
                    e.initEvent('b', true);
                    __inca_native__.dispatchEvent({child}, e);
                    return out;
                }})()
                "
            ),
        );

        assert_eq!(got, serde_json::json!(["child", "outer"]));
    }

    #[test]
    fn init_event_clears_the_canceled_flag() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const e = new Event('a', { cancelable: true });
                e.preventDefault();
                e.initEvent('a', false, true);
                return e.defaultPrevented;
            })()
            ",
        );

        assert_eq!(got, serde_json::json!(false));
    }

    #[test]
    fn init_event_clears_the_stop_flag() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const e = new Event('a');
                e.stopPropagation();
                e.initEvent('a');
                return e.cancelBubble;
            })()
            ",
        );

        assert_eq!(got, serde_json::json!(false));
    }

    #[test]
    fn init_event_clears_the_immediate_stop_flag() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(&dispatcher, node, "a", "() => out.push('ran')");

        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                (() => {{
                    const e = new Event('a');
                    e.stopImmediatePropagation();
                    e.initEvent('a');
                    __inca_native__.dispatchEvent({node}, e);
                    return out;
                }})()
                "
            ),
        );

        assert_eq!(got, serde_json::json!(["ran"]));
    }

    #[gpui::test]
    fn init_event_makes_a_kept_host_event_untrusted(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(&dispatcher, node, "click", "(e) => (globalThis.kept = e)");
        fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const before = kept.isTrusted;
                kept.initEvent('zz');
                return [before, kept.isTrusted];
            })()
            ",
        );

        assert_eq!(got, serde_json::json!([true, false]));
    }

    #[test]
    fn init_event_keeps_composed() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const e = new Event('a', { composed: true });
                e.initEvent('b');
                return e.composed;
            })()
            ",
        );

        assert_eq!(got, serde_json::json!(true));
    }

    #[test]
    fn init_event_keeps_the_target() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();

        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                (() => {{
                    const e = new Event('a');
                    __inca_native__.dispatchEvent({node}, e);
                    e.initEvent('b');
                    return e.target;
                }})()
                "
            ),
        );

        assert_eq!(got, serde_json::json!(node));
    }
}
