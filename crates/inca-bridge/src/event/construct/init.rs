// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The init dictionary that the event constructors read.

use gpui::Modifiers;
use rquickjs::convert::Coerced;
use rquickjs::{Ctx, Exception, Object, Result as JsResult, Value};

use crate::event::payload::{Input, Keyboard, Mouse, Payload, Pointer, Ui, Wheel};

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
pub(super) fn mouse<'js>(init: &Init<'_, 'js>, ui: Ui) -> JsResult<(Mouse, Option<Value<'js>>)> {
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
        // A script-built event takes -1 as 0.
        button: if button == -1 { 0 } else { button },
        buttons,
        modifiers,
    };
    Ok((mouse, related))
}

/// The pointer fields of `init`.
pub(super) fn pointer(init: &Init<'_, '_>) -> JsResult<Pointer> {
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
pub(super) fn wheel(init: &Init<'_, '_>) -> JsResult<Wheel> {
    let mode = init.unsigned_long("deltaMode")?;
    let x = init.double("deltaX", 0.0)?;
    let y = init.double("deltaY", 0.0)?;
    let z = init.double("deltaZ", 0.0)?;
    Ok(Wheel { x, y, z, mode })
}

/// The payload of a `KeyboardEvent` built from `init`. `which` is `keyCode`.
pub(super) fn keyboard(init: &Init<'_, '_>, ui: Ui) -> JsResult<Payload> {
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

/// The payload of an `InputEvent` built from `init`. The `dataTransfer` and `targetRanges` members
/// are not read.
pub(super) fn input(init: &Init<'_, '_>, ui: Ui) -> JsResult<Payload> {
    let data = match init.member("data")? {
        Some(value) if !value.is_null() => Some(value.get::<Coerced<String>>()?.0),
        _ => None,
    };
    let input_type = init.string("inputType")?;
    let is_composing = init.boolean("isComposing")?;
    Ok(Payload::Input(
        ui,
        Input {
            data,
            kind: input_type,
            is_composing,
        },
    ))
}

/// The init dictionary an event constructor reads its members from.
pub(super) struct Init<'a, 'js> {
    pub(super) ctx: &'a Ctx<'js>,
    /// The class being built, for the messages.
    pub(super) name: &'static str,
    pub(super) object: Option<Object<'js>>,
}

impl<'js> Init<'_, 'js> {
    /// Member `key`, `None` when the init is absent or the member is `undefined`.
    pub(super) fn member(&self, key: &str) -> JsResult<Option<Value<'js>>> {
        let Some(object) = &self.object else {
            return Ok(None);
        };
        let value: Value = object.get(key)?;
        Ok((!value.is_undefined()).then_some(value))
    }

    pub(super) fn boolean(&self, key: &str) -> JsResult<bool> {
        match self.member(key)? {
            Some(value) => Ok(value.get::<Coerced<bool>>()?.0),
            None => Ok(false),
        }
    }

    /// A `long` member: the value converted with `ToInt32`.
    pub(super) fn long(&self, key: &str) -> JsResult<i32> {
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
    pub(super) fn string(&self, key: &str) -> JsResult<String> {
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
    pub(super) fn target(&self, key: &str) -> JsResult<Option<Value<'js>>> {
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::tests::{dispatcher_with_engine, eval_json, listen};
    use rstest::rstest;

    #[rstest]
    #[case::long("new UIEvent('x', { detail: 2 ** 33 + 1 }).detail", serde_json::json!(1))]
    #[case::short("new MouseEvent('x', { button: 70000 }).button", serde_json::json!(4464))]
    #[case::unsigned_short("new MouseEvent('x', { buttons: -1 }).buttons", serde_json::json!(65535))]
    #[case::unsigned_long(
        "new KeyboardEvent('x', { keyCode: -1 }).keyCode",
        serde_json::json!(4294967295u32)
    )]
    #[case::client_coordinate("new MouseEvent('x', { clientX: -1.5 }).clientX", serde_json::json!(-2))]
    #[case::float(
        "new PointerEvent('x', { pressure: 0.3 }).pressure",
        serde_json::json!(0.30000001192092896)
    )]
    #[case::double_default("new PointerEvent('x', { width: undefined }).width", serde_json::json!(1))]
    #[case::pressure_default("new PointerEvent('x').pressure", serde_json::json!(0))]
    #[case::height_default("new PointerEvent('x').height", serde_json::json!(1))]
    #[case::string("new KeyboardEvent('x', { key: 5 }).key", serde_json::json!("5"))]
    #[case::string_default("new PointerEvent('x').pointerType", serde_json::json!(""))]
    #[case::null_target(
        "new FocusEvent('x', { relatedTarget: null }).relatedTarget",
        serde_json::json!(null)
    )]
    fn an_init_member_converts_by_its_idl_type(
        #[case] expression: &str,
        #[case] want: serde_json::Value,
    ) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        assert_eq!(eval_json(&dispatcher, expression), want);
    }

    #[rstest]
    #[case::not_finite("new MouseEvent('x', { clientX: NaN })")]
    #[case::float_overflow("new PointerEvent('x', { pressure: 1e39 })")]
    #[case::primitive_target("new MouseEvent('x', { relatedTarget: 5 })")]
    fn a_bad_init_member_throws_a_type_error(#[case] expression: &str) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                (() => {{
                    try {{
                        {expression};
                        return 'ran';
                    }} catch (err) {{
                        return err instanceof TypeError;
                    }}
                }})()
                "
            ),
        );

        assert_eq!(got, serde_json::json!(true));
    }

    #[test]
    fn a_ui_event_reads_its_init() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const e = new UIEvent('x', { detail: 3, which: 7 });
                return [e.detail, e.which, e.view];
            })()
            ",
        );

        assert_eq!(got, serde_json::json!([3, 0, null]));
    }

    #[test]
    fn the_which_of_a_built_mouse_event_is_its_button_plus_one() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(&dispatcher, "new MouseEvent('x', { button: -2 }).which");

        assert_eq!(got, serde_json::json!(4294967295u32));
    }

    #[test]
    fn a_built_mouse_event_reads_button_minus_one_as_zero() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(&dispatcher, "new MouseEvent('x', { button: -1 }).button");

        assert_eq!(got, serde_json::json!(0));
    }

    #[test]
    fn a_built_mouse_event_copies_its_client_position() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const e = new MouseEvent('x', { clientX: 25.5, clientY: 37 });
                return [e.pageX, e.pageY, e.x, e.y, e.offsetX, e.offsetY];
            })()
            ",
        );

        assert_eq!(got, serde_json::json!([25, 37, 25, 37, 25, 37]));
    }

    #[test]
    fn a_built_mouse_event_floors_its_screen_position() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const e = new MouseEvent('x', { screenX: 3.7, screenY: -4.5, clientX: 1, clientY: 2 });
                return [e.screenX, e.screenY];
            })()
            ",
        );

        assert_eq!(got, serde_json::json!([3, -5]));
    }

    #[test]
    fn a_built_mouse_event_truncates_its_movement() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const e = new MouseEvent('x', { movementX: -2.5, movementY: NaN });
                return [e.movementX, e.movementY];
            })()
            ",
        );

        assert_eq!(got, serde_json::json!([-2, 0]));
    }

    #[rstest]
    #[case::alt("altKey", serde_json::json!([true, false, false, false]))]
    #[case::ctrl("ctrlKey", serde_json::json!([false, true, false, false]))]
    #[case::meta("metaKey", serde_json::json!([false, false, true, false]))]
    #[case::shift("shiftKey", serde_json::json!([false, false, false, true]))]
    fn a_built_mouse_event_reads_its_modifier_keys(
        #[case] key: &str,
        #[case] want: serde_json::Value,
    ) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                (() => {{
                    const e = new MouseEvent('x', {{ '{key}': 1 }});
                    return [e.altKey, e.ctrlKey, e.metaKey, e.shiftKey];
                }})()
                "
            ),
        );

        assert_eq!(got, want);
    }

    #[test]
    fn a_pointer_event_reads_its_init() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const e = new PointerEvent('pointerdown', {
                    pointerId: 7,
                    width: 2.5,
                    height: 3,
                    pressure: 0.25,
                    pointerType: 'pen',
                    isPrimary: true,
                });
                return [e.pointerId, e.width, e.height, e.pressure, e.pointerType, e.isPrimary];
            })()
            ",
        );

        assert_eq!(got, serde_json::json!([7, 2.5, 3, 0.25, "pen", true]));
    }

    #[test]
    fn a_wheel_event_reads_its_init() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const e = new WheelEvent('wheel', { deltaX: 0.1, deltaY: 2, deltaZ: 3, deltaMode: 1.9 });
                return [e.deltaX, e.deltaY, e.deltaZ, e.deltaMode];
            })()
            ",
        );

        assert_eq!(got, serde_json::json!([0.1, 2, 3, 1]));
    }

    #[test]
    fn a_keyboard_event_reads_its_init() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const e = new KeyboardEvent('keydown', {
                    key: 'Enter',
                    code: 'NumpadEnter',
                    location: 3,
                    repeat: true,
                    isComposing: false,
                    keyCode: 13,
                    charCode: 65,
                    which: 9,
                    shiftKey: true,
                });
                return [
                    e.key,
                    e.code,
                    e.location,
                    e.repeat,
                    e.isComposing,
                    e.keyCode,
                    e.charCode,
                    e.which,
                    e.getModifierState('Shift'),
                ];
            })()
            ",
        );

        assert_eq!(
            got,
            serde_json::json!(["Enter", "NumpadEnter", 3, true, false, 13, 65, 13, true])
        );
    }

    #[rstest]
    #[case::defaults("'x'", serde_json::json!([null, "", false, null, 0]))]
    #[case::given(
        "'x', { data: 5, inputType: 'insertText', isComposing: 1 }",
        serde_json::json!(["5", "insertText", true, null, 0])
    )]
    #[case::null_data("'x', { data: null }", serde_json::json!([null, "", false, null, 0]))]
    fn new_input_event_reads_its_init(#[case] args: &str, #[case] want: serde_json::Value) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let js = r"
            (() => {
                const e = new InputEvent(ARGS);
                return [e.data, e.inputType, e.isComposing, e.dataTransfer, e.getTargetRanges().length];
            })()
            "
        .replace("ARGS", args);

        assert_eq!(eval_json(&dispatcher, &js), want);
    }

    #[rstest]
    #[case::defaults("new CompositionEvent('x').data", serde_json::json!(""))]
    #[case::given("new CompositionEvent('x', { data: 7 }).data", serde_json::json!("7"))]
    fn new_composition_event_reads_its_data(
        #[case] expression: &str,
        #[case] want: serde_json::Value,
    ) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        assert_eq!(eval_json(&dispatcher, expression), want);
    }

    #[test]
    fn get_target_ranges_returns_a_new_empty_array() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const e = new InputEvent('x');
                const first = e.getTargetRanges();
                const second = e.getTargetRanges();
                return [Array.isArray(first), first.length, first === second];
            })()
            ",
        );

        assert_eq!(got, serde_json::json!([true, 0, false]));
    }

    #[test]
    fn a_keyboard_event_reads_its_composing_state() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const e = new KeyboardEvent('keydown', { repeat: false, isComposing: true });
                return [e.repeat, e.isComposing];
            })()
            ",
        );

        assert_eq!(got, serde_json::json!([false, true]));
    }

    #[test]
    fn a_built_mouse_event_shows_its_fields_to_a_listener() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "click",
            "(e) => out.push([e.clientX, e.button])",
        );

        let got = eval_json(
            &dispatcher,
            &format!(
                concat!(
                    "(__inca_native__.dispatchEvent({node}, ",
                    "new MouseEvent('click', {{ clientX: 5, button: 2 }})), out)"
                ),
                node = node,
            ),
        );

        assert_eq!(got, serde_json::json!([[5, 2]]));
    }
}
