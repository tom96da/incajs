// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The walk of the `abort` event of a signal.

use std::cell::RefCell;
use std::rc::Rc;

use inca_document::EventPayload;
use rquickjs::Ctx;

use super::{Flags, Walk, listened, next_event_id, run};
use crate::EventTargetId;
use crate::bindings::Host;

/// Fires the trusted `abort` event at signal `id`. The event does not bubble and cannot be
/// canceled. It enters the engine only when a listener of the signal exists. Listener errors go to
/// `Host::failures`.
pub(crate) fn fire_abort(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>, id: u32) {
    let walk = Walk {
        name: "abort",
        flags: Flags::NONE,
        target: EventTargetId::Signal(id),
        cut: &[],
        payload: &EventPayload::None,
        trusted: true,
    };
    if listened(&host.borrow(), walk.name, walk.flags, walk.target, walk.cut) {
        run(ctx, host, &walk, next_event_id(host));
    }
}
