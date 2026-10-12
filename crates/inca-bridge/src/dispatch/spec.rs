// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::event::Flags;

/// The rules of one event name.
#[derive(Clone, Copy)]
pub(crate) struct EventSpec {
    name: &'static str,
    pub(crate) flags: Flags,
    /// Takes `movementX`/`movementY` from the raw pointer move.
    pub(super) movement: bool,
    /// Carries the `buttons` of its own press or release.
    pub(super) own_buttons: bool,
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
pub(super) const EVENTS: [EventSpec; 19] = [
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
    spec("wheel", Flags::ALL),
    spec("scroll", Flags::NONE),
    spec("keydown", Flags::ALL),
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
];

/// The rules of `event`. Any other name takes all three flags as false.
pub(crate) fn spec_of(event: &str) -> EventSpec {
    EVENTS
        .iter()
        .find(|spec| spec.name == event)
        .copied()
        .unwrap_or(spec("", Flags::NONE))
}
