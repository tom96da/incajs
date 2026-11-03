// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The flags and rules of each event name.

use super::{EventPayload, InputPayload, InputType};

/// `bubbles`, `cancelable` and `composed` of one event.
#[derive(Clone, Copy)]
pub struct Flags {
    pub bubbles: bool,
    pub cancelable: bool,
    pub composed: bool,
}

impl Flags {
    pub const ALL: Self = Self {
        bubbles: true,
        cancelable: true,
        composed: true,
    };
    pub const NONE: Self = Self {
        bubbles: false,
        cancelable: false,
        composed: false,
    };
    /// Focus and capture events: composed and not cancelable.
    pub const FOCUS: Self = Self {
        composed: true,
        ..Self::NONE
    };
}

/// The rules of one event name.
#[derive(Clone, Copy)]
pub struct EventSpec {
    /// The event name.
    pub name: &'static str,
    pub flags: Flags,
    /// Takes `movementX`/`movementY` from the raw pointer move.
    pub movement: bool,
    /// Carries the `buttons` of its own press or release.
    pub own_buttons: bool,
}

const fn spec(name: &'static str, flags: Flags) -> EventSpec {
    EventSpec {
        name,
        flags,
        movement: false,
        own_buttons: false,
    }
}

const fn own_buttons(mut event: EventSpec) -> EventSpec {
    event.own_buttons = true;
    event
}

/// Every event the host dispatches.
pub const EVENTS: [EventSpec; 35] = [
    own_buttons(spec("click", Flags::ALL)),
    own_buttons(spec("dblclick", Flags::ALL)),
    own_buttons(spec("auxclick", Flags::ALL)),
    own_buttons(spec("contextmenu", Flags::ALL)),
    spec("mousedown", Flags::ALL),
    spec("mouseup", Flags::ALL),
    EventSpec {
        movement: true,
        ..spec("mousemove", Flags::ALL)
    },
    spec("mouseenter", Flags::NONE),
    spec("mouseleave", Flags::NONE),
    spec("mouseover", Flags::ALL),
    spec("mouseout", Flags::ALL),
    spec("pointerdown", Flags::ALL),
    spec("pointerup", Flags::ALL),
    EventSpec {
        movement: true,
        ..spec("pointermove", Flags::ALL)
    },
    spec("pointerover", Flags::ALL),
    spec("pointerout", Flags::ALL),
    spec("pointerenter", Flags::NONE),
    spec("pointerleave", Flags::NONE),
    spec(
        "gotpointercapture",
        Flags {
            bubbles: true,
            ..Flags::FOCUS
        },
    ),
    spec(
        "lostpointercapture",
        Flags {
            bubbles: true,
            ..Flags::FOCUS
        },
    ),
    spec("wheel", Flags::ALL),
    spec("scroll", Flags::NONE),
    spec("keydown", Flags::ALL),
    spec("keypress", Flags::ALL),
    spec("keyup", Flags::ALL),
    spec("focus", Flags::FOCUS),
    spec("blur", Flags::FOCUS),
    spec(
        "focusin",
        Flags {
            bubbles: true,
            ..Flags::FOCUS
        },
    ),
    spec(
        "focusout",
        Flags {
            bubbles: true,
            ..Flags::FOCUS
        },
    ),
    spec("beforeinput", Flags::ALL),
    spec(
        "input",
        Flags {
            bubbles: true,
            ..Flags::FOCUS
        },
    ),
    spec(
        "change",
        Flags {
            bubbles: true,
            ..Flags::NONE
        },
    ),
    spec("compositionstart", Flags::ALL),
    spec("compositionupdate", Flags::ALL),
    spec("compositionend", Flags::ALL),
];

/// The rules of `event`. Any other name takes all three flags as false.
#[must_use]
pub fn spec_of(event: &str) -> EventSpec {
    EVENTS
        .iter()
        .find(|spec| spec.name == event)
        .copied()
        .unwrap_or(spec("", Flags::NONE))
}

/// The flags of `event` carrying `payload`: those of [`spec_of`], with a
/// `beforeinput` of `insertCompositionText` not cancelable.
#[must_use]
pub fn flags_of(event: &str, payload: &EventPayload) -> Flags {
    let mut flags = spec_of(event).flags;
    if event == "beforeinput"
        && matches!(
            payload,
            EventPayload::Input(InputPayload {
                input_type: InputType::InsertCompositionText,
                ..
            })
        )
    {
        flags.cancelable = false;
    }
    flags
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;
    use rstest::rstest;

    fn input_payload(input_type: InputType) -> EventPayload {
        EventPayload::Input(InputPayload {
            data: None,
            input_type,
            is_composing: false,
        })
    }

    #[rstest]
    #[case::down("pointerdown", true, true, true, false)]
    #[case::up("pointerup", true, true, true, false)]
    #[case::moved("pointermove", true, true, true, true)]
    #[case::over("pointerover", true, true, true, false)]
    #[case::out("pointerout", true, true, true, false)]
    #[case::enter("pointerenter", false, false, false, false)]
    #[case::leave("pointerleave", false, false, false, false)]
    #[case::got("gotpointercapture", true, false, true, false)]
    #[case::lost("lostpointercapture", true, false, true, false)]
    fn pointer_event_specs_match_the_standard_flags(
        #[case] name: &str,
        #[case] bubbles: bool,
        #[case] cancelable: bool,
        #[case] composed: bool,
        #[case] movement: bool,
    ) {
        let spec = spec_of(name);
        assert_eq!(spec.name, name);
        assert_eq!(
            (
                spec.flags.bubbles,
                spec.flags.cancelable,
                spec.flags.composed,
                spec.movement
            ),
            (bubbles, cancelable, composed, movement)
        );
    }

    #[rstest]
    #[case::composition("beforeinput", input_payload(InputType::InsertCompositionText), false)]
    #[case::insert_text("beforeinput", input_payload(InputType::InsertText), true)]
    #[case::none_payload("beforeinput", EventPayload::None, true)]
    #[case::other_event("keydown", input_payload(InputType::InsertCompositionText), true)]
    fn flags_of_drops_cancelable_for_a_composition_beforeinput(
        #[case] event: &str,
        #[case] payload: EventPayload,
        #[case] cancelable: bool,
    ) {
        assert_eq!(flags_of(event, &payload).cancelable, cancelable);
    }
}
