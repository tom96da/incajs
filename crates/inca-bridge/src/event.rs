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

mod construct;
mod construct_init;
mod listeners;
mod members;
mod object;
mod payload;
mod record;
mod walk;
mod walk_click;
mod walk_listeners;
mod walk_script;
mod walk_targets;

pub(crate) use listeners::install_events;
use object::{
    COMPOSITION, CUSTOM, EVENT, FOCUS, INPUT, KEYBOARD, MOUSE, POINTER, UI, WHEEL, child_of,
    constants, given, initialize, new_event, other_kind, with_event,
};
pub(crate) use object::{array_of, install};
use record::{Held, IncaEvent, State};
use walk::targets_of;
pub(crate) use walk::{click, dispatch_event, fire_abort, lose_focus, walk};
pub(crate) use walk_targets::{HookTarget, WINDOW_TAG};
