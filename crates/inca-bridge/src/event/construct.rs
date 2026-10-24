// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;

use gpui::Modifiers;
use inca_gpui::{EventPayload, NodeId};
use rquickjs::atom::PredefinedAtom;
use rquickjs::convert::Coerced;
use rquickjs::function::{IntoJsFunc, Opt, ParamRequirement, Params, This};
use rquickjs::object::Property;
use rquickjs::{Class, Ctx, Exception, Function, Object, Result as JsResult, Value};

use super::members::PHASES;
use super::payload::{DELTA_MODES, KEY_LOCATIONS, Keyboard, Mouse, Payload, Pointer, Ui, Wheel};
use super::walk::Walk;
use super::{
    CUSTOM, EVENT, FOCUS, Flags, Held, IncaEvent, KEYBOARD, MOUSE, POINTER, State, UI, WHEEL,
    child_of, constants, field, given, initialize, method, new_event, other_kind, targets_of,
    with_event,
};
use crate::EventTargetId;
use crate::bindings::convert::or_null;

/// Each class of event: the kind it builds, its name, and the kind whose constructor its
/// constructor inherits from.
pub(super) const CLASSES: [(&str, &str, Option<&str>); 8] = [
    (EVENT, "Event", None),
    (CUSTOM, "CustomEvent", Some(EVENT)),
    (UI, "UIEvent", Some(EVENT)),
    (FOCUS, "FocusEvent", Some(UI)),
    (MOUSE, "MouseEvent", Some(UI)),
    (POINTER, "PointerEvent", Some(MOUSE)),
    (WHEEL, "WheelEvent", Some(MOUSE)),
    (KEYBOARD, "KeyboardEvent", Some(UI)),
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
    let state = State::new(walk.name.to_owned(), walk.flags, payload, walk.trusted);
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
        EventPayload::None | EventPayload::Key(_) => None,
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
    method(&proto, "initCustomEvent", init)?;
    Ok(proto)
}

/// Installs a global constructor for each of [`CLASSES`], linked with the prototype of the same
/// index in `prototypes`, and the constants of `Event`, `WheelEvent` and `KeyboardEvent`.
pub(super) fn constructors<'js>(ctx: &Ctx<'js>, prototypes: &[Object<'js>; 8]) -> JsResult<()> {
    let globals = ctx.globals();
    let mut made: Vec<(&str, Function<'js>)> = Vec::new();
    for (&(kind, name, parent), proto) in CLASSES.iter().zip(prototypes) {
        let function = constructor(ctx, proto, name, kind)?;
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

/// The constructor `name` of length 1 for events of `kind`, linked with `proto` as
/// `JS_SetConstructor` links a built-in: `name.prototype` is read-only and `proto.constructor`
/// is writable and configurable. `proto` gets the string tag `name`.
fn constructor<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    name: &'static str,
    kind: &'static str,
) -> JsResult<Function<'js>> {
    let function = Function::new(ctx.clone(), Construct { name, kind })?
        .with_name(name)?
        .with_constructor(true);
    function.set_length(1)?;
    function.prop("prototype", Property::from(proto.clone()))?;
    proto.prop(
        "constructor",
        Property::from(function.clone()).writable().configurable(),
    )?;
    proto.prop(
        PredefinedAtom::SymbolToStringTag,
        Property::from(name).configurable(),
    )?;
    Ok(function)
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
        let state = State::new(event_type, flags, payload, false);
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
        // `new.target`'s prototype when it is an object, as for a subclass;
        // the prototype of the kind otherwise.
        let target_proto = params
            .this()
            .as_object()
            .map(|target| target.get::<_, Value>("prototype"))
            .transpose()?
            .and_then(Value::into_object);
        if let Some(proto) = target_proto {
            event.set_prototype(Some(&proto))?;
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

/// The modifier members of `init`: `altKey`, `ctrlKey`, `metaKey`, `shiftKey`.
fn modifiers(init: &Init<'_, '_>) -> JsResult<Modifiers> {
    let alt = init.boolean("altKey")?;
    let control = init.boolean("ctrlKey")?;
    let platform = init.boolean("metaKey")?;
    let shift = init.boolean("shiftKey")?;
    Ok(Modifiers {
        control,
        alt,
        shift,
        platform,
        function: false,
    })
}

/// The mouse fields of `init` and its `relatedTarget`. `ui` holds the `detail` read before.
fn mouse<'js>(init: &Init<'_, 'js>, ui: Ui) -> JsResult<(Mouse, Option<Value<'js>>)> {
    let modifiers = modifiers(init)?;
    let button = init.short("button")?;
    let buttons = init.unsigned_short("buttons")?;
    let client_x = init.double("clientX", 0.0)?.floor();
    let client_y = init.double("clientY", 0.0)?.floor();
    let movement_x = f64::from(init.long("movementX")?);
    let movement_y = f64::from(init.long("movementY")?);
    let related = init.target("relatedTarget")?;
    let screen_x = init.double("screenX", 0.0)?.floor();
    let screen_y = init.double("screenY", 0.0)?.floor();
    // `which` is an unsigned long, so a button below -1 wraps.
    #[allow(clippy::cast_sign_loss)]
    let which = (i32::from(button) + 1) as u32;
    let mouse = Mouse {
        ui: Ui { which, ..ui },
        screen_x,
        screen_y,
        client_x,
        client_y,
        offset_x: client_x,
        offset_y: client_y,
        movement_x,
        movement_y,
        button,
        buttons,
        modifiers,
    };
    Ok((mouse, related))
}

/// The pointer fields of `init`.
fn pointer(init: &Init<'_, '_>) -> JsResult<Pointer> {
    let height = init.double("height", 1.0)?;
    let is_primary = init.boolean("isPrimary")?;
    let id = init.long("pointerId")?;
    let kind = init.string("pointerType")?;
    let pressure = init.float("pressure", 0.0)?;
    let width = init.double("width", 1.0)?;
    Ok(Pointer {
        id,
        width,
        height,
        pressure,
        kind,
        is_primary,
    })
}

/// The wheel fields of `init`.
fn wheel(init: &Init<'_, '_>) -> JsResult<Wheel> {
    let mode = init.unsigned_long("deltaMode")?;
    let x = init.double("deltaX", 0.0)?;
    let y = init.double("deltaY", 0.0)?;
    let z = init.double("deltaZ", 0.0)?;
    Ok(Wheel { x, y, z, mode })
}

/// The payload of a `KeyboardEvent` built from `init`. `which` is `keyCode`.
fn keyboard(init: &Init<'_, '_>, ui: Ui) -> JsResult<Payload> {
    let modifiers = modifiers(init)?;
    let char_code = init.unsigned_long("charCode")?;
    let code = init.string("code")?;
    let is_composing = init.boolean("isComposing")?;
    let key = init.string("key")?;
    let key_code = init.unsigned_long("keyCode")?;
    let location = init.unsigned_long("location")?;
    let repeat = init.boolean("repeat")?;
    Ok(Payload::Keyboard(
        Ui {
            which: key_code,
            ..ui
        },
        Keyboard {
            key,
            code,
            location,
            repeat,
            is_composing,
            modifiers,
            key_code,
            char_code,
        },
    ))
}

/// The init dictionary an event constructor reads its members from.
struct Init<'a, 'js> {
    ctx: &'a Ctx<'js>,
    /// The class being built, for the messages.
    name: &'static str,
    object: Option<Object<'js>>,
}

impl<'js> Init<'_, 'js> {
    /// Member `key`, `None` when the init is absent or the member is `undefined`.
    fn member(&self, key: &str) -> JsResult<Option<Value<'js>>> {
        let Some(object) = &self.object else {
            return Ok(None);
        };
        let value: Value = object.get(key)?;
        Ok((!value.is_undefined()).then_some(value))
    }

    fn boolean(&self, key: &str) -> JsResult<bool> {
        match self.member(key)? {
            Some(value) => Ok(value.get::<Coerced<bool>>()?.0),
            None => Ok(false),
        }
    }

    /// A `long` member: the value converted with `ToInt32`.
    fn long(&self, key: &str) -> JsResult<i32> {
        match self.member(key)? {
            Some(value) => Ok(value.get::<Coerced<i32>>()?.0),
            None => Ok(0),
        }
    }

    /// An `unsigned long` member: the 32 bits of [`Self::long`].
    #[allow(clippy::cast_sign_loss)]
    fn unsigned_long(&self, key: &str) -> JsResult<u32> {
        Ok(self.long(key)? as u32)
    }

    /// A `short` member: the low 16 bits of [`Self::long`].
    #[allow(clippy::cast_possible_truncation)]
    fn short(&self, key: &str) -> JsResult<i16> {
        Ok(self.long(key)? as i16)
    }

    /// An `unsigned short` member: the low 16 bits of [`Self::long`].
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn unsigned_short(&self, key: &str) -> JsResult<u16> {
        Ok(self.long(key)? as u16)
    }

    /// A `double` member, `default` when absent.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for a value that is not a finite number.
    fn double(&self, key: &str, default: f64) -> JsResult<f64> {
        let Some(value) = self.member(key)? else {
            return Ok(default);
        };
        let number = value.get::<Coerced<f64>>()?.0;
        if number.is_finite() {
            Ok(number)
        } else {
            Err(self.not_finite(key))
        }
    }

    /// A `float` member, `default` when absent.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for a value that is not a finite number as an `f32`.
    fn float(&self, key: &str, default: f32) -> JsResult<f32> {
        #[allow(clippy::cast_possible_truncation)]
        let number = self.double(key, f64::from(default))? as f32;
        if number.is_finite() {
            Ok(number)
        } else {
            Err(self.not_finite(key))
        }
    }

    /// A `DOMString` member, `""` when absent.
    fn string(&self, key: &str) -> JsResult<String> {
        match self.member(key)? {
            Some(value) => Ok(value.get::<Coerced<String>>()?.0),
            None => Ok(String::new()),
        }
    }

    /// An `EventTarget?` member: an object, or `None` for `null`.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for any other value.
    fn target(&self, key: &str) -> JsResult<Option<Value<'js>>> {
        match self.member(key)? {
            Some(value) if value.is_object() => Ok(Some(value)),
            Some(value) if !value.is_null() => Err(self.error(&format!(
                "the {key} member is not an object, so pass an element, document, window or null"
            ))),
            _ => Ok(None),
        }
    }

    fn not_finite(&self, key: &str) -> rquickjs::Error {
        self.error(&format!(
            "the {key} member is not a finite number, so pass a finite number or leave it out"
        ))
    }

    fn error(&self, message: &str) -> rquickjs::Error {
        Exception::throw_type(
            self.ctx,
            &format!("cannot construct {}: {message}", self.name),
        )
    }
}
