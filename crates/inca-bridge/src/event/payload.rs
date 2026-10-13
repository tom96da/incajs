// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::bindings::convert::or_null;
use inca_gpui::{EventPayload, KeyPayload, MousePayload, PointerSource};
use rquickjs::{Function, Object, Result as JsResult};

/// Writes `payload`'s fields onto `event_object`.
pub(super) fn set_payload(event_object: &Object, payload: &EventPayload) -> JsResult<()> {
    match payload {
        EventPayload::None => {}
        EventPayload::Mouse(mouse) => set_mouse_fields(event_object, mouse)?,
        EventPayload::Wheel(wheel) => {
            set_mouse_fields(event_object, &wheel.mouse)?;
            event_object.set("deltaX", wheel.delta_x)?;
            event_object.set("deltaY", wheel.delta_y)?;
            event_object.set("deltaZ", wheel.delta_z)?;
            event_object.set("deltaMode", wheel.delta_mode)?;
        }
        EventPayload::Key(key) => set_key_fields(event_object, key)?,
        EventPayload::Focus { related_target } => {
            event_object.set(
                "relatedTarget",
                or_null(event_object.ctx(), *related_target)?,
            )?;
        }
    }
    Ok(())
}

/// Writes [`MousePayload`]'s fields onto `event_object`. Shared by
/// [`EventPayload::Mouse`] and [`EventPayload::Wheel`] (a wheel payload
/// carries every mouse field).
fn set_mouse_fields(event_object: &Object, mouse: &MousePayload) -> JsResult<()> {
    event_object.set("clientX", mouse.client_x)?;
    event_object.set("clientY", mouse.client_y)?;
    event_object.set("screenX", mouse.screen_x)?;
    event_object.set("screenY", mouse.screen_y)?;
    event_object.set("offsetX", mouse.offset_x)?;
    event_object.set("offsetY", mouse.offset_y)?;
    event_object.set("x", mouse.client_x)?;
    event_object.set("y", mouse.client_y)?;
    event_object.set(
        "relatedTarget",
        or_null(event_object.ctx(), mouse.related_target)?,
    )?;
    // Identical to clientX/clientY today. Nothing here scrolls the page
    // itself, which is the only thing that would tell them apart.
    event_object.set("pageX", mouse.client_x)?;
    event_object.set("pageY", mouse.client_y)?;
    event_object.set("movementX", mouse.movement_x)?;
    event_object.set("movementY", mouse.movement_y)?;
    event_object.set("button", mouse.button)?;
    event_object.set("buttons", mouse.buttons)?;
    event_object.set("detail", mouse.detail)?;
    if let Some(source) = mouse.pointer {
        let (id, kind, primary) = match source {
            PointerSource::Mouse => (1, "mouse", true),
            PointerSource::Keyboard => (-1, "", false),
        };
        event_object.set("pointerId", id)?;
        event_object.set("pointerType", kind)?;
        event_object.set("isPrimary", primary)?;
        event_object.set("width", 1)?;
        event_object.set("height", 1)?;
        // Pointer events report 0.5 while a button is held.
        event_object.set("pressure", if mouse.buttons == 0 { 0.0 } else { 0.5 })?;
    }
    set_modifier_fields(event_object, mouse.modifiers)
}

/// Writes [`KeyPayload`]'s fields onto `event_object`.
fn set_key_fields(event_object: &Object, key: &KeyPayload) -> JsResult<()> {
    event_object.set("key", key.key.clone())?;
    event_object.set("repeat", key.repeat)?;
    // gpui reports no key location or composition state.
    event_object.set("location", 0)?;
    event_object.set("isComposing", false)?;
    set_modifier_fields(event_object, key.modifiers)
}

/// Writes `modifiers`' fields onto `event_object`. Shared by
/// [`set_mouse_fields`] and [`set_key_fields`].
fn set_modifier_fields(event_object: &Object, modifiers: gpui::Modifiers) -> JsResult<()> {
    event_object.set("ctrlKey", modifiers.control)?;
    event_object.set("shiftKey", modifiers.shift)?;
    event_object.set("altKey", modifiers.alt)?;
    event_object.set("metaKey", modifiers.platform)?;
    event_object.set(
        "getModifierState",
        Function::new(event_object.ctx().clone(), move |name: String| {
            modifier_state(modifiers, &name)
        })?,
    )?;
    Ok(())
}

/// Whether the modifier `name` is held. `Accel` is Meta on macOS and Control
/// elsewhere. Other names return false.
fn modifier_state(modifiers: gpui::Modifiers, name: &str) -> bool {
    match name {
        "Control" => modifiers.control,
        "Shift" => modifiers.shift,
        "Alt" => modifiers.alt,
        "Meta" => modifiers.platform,
        "Accel" => {
            if cfg!(target_os = "macos") {
                modifiers.platform
            } else {
                modifiers.control
            }
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifier_state_reports_each_held_modifier_and_no_lock() {
        let mut mismatches = Vec::new();
        for (name, held) in [
            ("Control", gpui::Modifiers::control()),
            ("Shift", gpui::Modifiers::shift()),
            ("Alt", gpui::Modifiers::alt()),
            ("Meta", gpui::Modifiers::command()),
        ] {
            for other in ["Control", "Shift", "Alt", "Meta", "CapsLock", "NumLock"] {
                if modifier_state(held, other) != (other == name) {
                    mismatches.push(format!("{name} held, {other} queried"));
                }
            }
        }
        let accel = if cfg!(target_os = "macos") {
            gpui::Modifiers::command()
        } else {
            gpui::Modifiers::control()
        };
        if !modifier_state(accel, "Accel") || modifier_state(gpui::Modifiers::default(), "Accel") {
            mismatches.push("Accel".to_string());
        }
        assert!(mismatches.is_empty(), "{mismatches:?}");
    }
}
