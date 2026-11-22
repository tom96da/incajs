// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Calls a listener of a JS event object and leaves the event idle.

use std::rc::Rc;

use inca_jsenv::callbacks;
use rquickjs::{Class, Ctx, Value};

use super::{IncaEvent, note};
use crate::bindings::Host;
use inca_document::{EventTargetId, Invoke, Listener};

/// Calls a listener of a JS event object in the engine context of the walk.
pub(super) struct JsInvoke<'a, 'js> {
    pub(super) ctx: &'a Ctx<'js>,
    pub(super) host: &'a Rc<Host>,
    pub(super) event: &'a Class<'js, IncaEvent<'js>>,
}

impl Invoke for JsInvoke<'_, '_> {
    fn stopped(&self) -> bool {
        self.event.borrow().state.stop.get()
    }

    fn immediately_stopped(&self) -> bool {
        self.event.borrow().state.immediate.get()
    }

    fn listeners(&self, target: EventTargetId, name: &str) -> Vec<Listener> {
        self.host
            .document
            .borrow()
            .listeners
            .listeners_for(target, name)
            .to_vec()
    }

    fn is_registered(&self, target: EventTargetId, name: &str, listener: &Listener) -> bool {
        self.host
            .document
            .borrow()
            .listeners
            .listeners_for(target, name)
            .contains(listener)
    }

    fn at(&self, index: usize, phase: u8) {
        let record = self.event.borrow();
        record.state.current.set(Some(index));
        record.state.phase.set(phase);
    }

    fn call(&self, listener: Listener) {
        let called = callbacks::get(self.ctx, listener.callback).and_then(|stored| {
            let Some(callback) = stored.and_then(Value::into_function) else {
                return Ok(());
            };
            self.event.borrow().state.passive.set(listener.passive);
            callback.call::<_, ()>((self.event.clone(),))
        });
        note(self.ctx, self.host, called);
    }

    fn end(&self) -> bool {
        let record = self.event.borrow();
        let canceled = record.state.canceled.get();
        end_dispatch(&record);
        canceled
    }
}

/// Leaves `event` as a dispatch leaves it: no walk state, no stop flags,
/// no path.
pub(super) fn end_dispatch(event: &IncaEvent<'_>) {
    let state = &event.state;
    for flag in [
        &state.dispatching,
        &state.passive,
        &state.stop,
        &state.immediate,
    ] {
        flag.set(false);
    }
    state.current.set(None);
    state.phase.set(0);
    // A kept event keeps only target, relatedTarget and detail alive.
    drop(event.held.path.take());
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::tests::{dispatcher_with_engine, listen, listen_with, tree_of_three};
    use inca_document::EventPayload;

    #[test]
    fn a_passive_listener_cannot_cancel_and_an_active_one_after_it_can() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        let passive = (false, true);
        listen_with(
            &dispatcher,
            parent,
            "click",
            (true, true),
            "(e) => { e.preventDefault(); out.push(e.defaultPrevented); }",
        );
        listen_with(
            &dispatcher,
            child,
            "click",
            passive,
            "(e) => { e.returnValue = false; out.push(e.defaultPrevented); }",
        );
        listen_with(
            &dispatcher,
            child,
            "click",
            passive,
            r"
            (e) => { e.stopPropagation(); e.preventDefault(); out.push(e.defaultPrevented); }
            ",
        );
        listen(
            &dispatcher,
            child,
            "click",
            r"
            (e) => {
                out.push(e.defaultPrevented); e.preventDefault();
                out.push(e.defaultPrevented);
            }
            ",
        );
        let canceled = dispatcher.fire(child, "click", &EventPayload::None);

        let out = dispatcher
            .engine
            .eval::<String>("JSON.stringify(out)")
            .unwrap();
        assert_eq!(out, "[false,false,false,false,true]");
        assert!(canceled);
    }

    #[test]
    fn an_event_only_passive_listeners_heard_stays_uncanceled() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, _, child] = tree_of_three(&host);
        listen_with(
            &dispatcher,
            child,
            "click",
            (false, true),
            "(e) => { e.preventDefault(); }",
        );
        let canceled = dispatcher.fire(child, "click", &EventPayload::None);

        assert!(!canceled);
    }
}
