// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The operations of the document on its listeners and its signals: adding and removing a
//! registration, the life of a signal, and which signals the signal table holds strongly.

use inca_node::NodeId;

use crate::{DocumentState, EventListeners, EventTargetId, Link, Signals};

/// The registration [`DocumentState::add_listener`] made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added {
    /// The callback id of the registration.
    pub callback: u32,
    /// The signals the signal table holds strongly from now on.
    pub pin: Vec<u32>,
}

/// What a change that drops registrations leaves to the JS side.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[must_use]
pub struct Released {
    /// The callback ids of the dropped registrations.
    pub callbacks: Vec<u32>,
    /// The signals the signal table stops holding strongly.
    pub unpin: Vec<u32>,
}

/// The signal [`DocumentState::create_any_signal`] made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnySignal {
    /// The id of the new signal.
    pub id: u32,
    /// The position of the first source that is aborted already. The new signal is then aborted
    /// and depends on no source; it takes the reason of that source.
    pub aborted_by: Option<usize>,
}

impl DocumentState {
    /// The listener registry, for reads.
    #[must_use]
    pub fn listeners(&self) -> &EventListeners {
        &self.listeners
    }

    /// The signal state, for reads.
    #[must_use]
    pub fn signals(&self) -> &Signals {
        &self.signals
    }

    /// Registers a listener for `event` on `target`, in the capture phase with `capture`, and
    /// removed when `signal` aborts. Returns `None` and calls nothing when `signal` is aborted
    /// already. Otherwise `callback` gives the callback id, and the result names the signals to
    /// hold strongly: `target` for an `abort` listener on a signal that pins, and `signal` when it
    /// pins.
    ///
    /// # Errors
    ///
    /// The error of `callback`, which leaves the document unchanged.
    pub fn add_listener<E>(
        &mut self,
        target: EventTargetId,
        event: String,
        capture: bool,
        passive: bool,
        signal: Option<u32>,
        callback: impl FnOnce() -> Result<u32, E>,
    ) -> Result<Option<Added>, E> {
        if signal.is_some_and(|signal| self.signals.is_aborted(signal)) {
            return Ok(None);
        }
        let callback = callback()?;
        let mut pin = Vec::new();
        if let EventTargetId::Signal(own) = target
            && event == "abort"
            && self.signals.pins(own)
        {
            pin.push(own);
        }
        let link = Link {
            target,
            event: event.clone(),
        };
        self.listeners
            .insert(target, event, callback, capture, passive);
        if let Some(signal) = signal {
            self.listeners.links.link(signal, callback, link);
            if self.signals.pins(signal) {
                pin.push(signal);
            }
        }
        Ok(Some(Added { callback, pin }))
    }

    /// Drops the registration of `callback` for `event` on `target`. Returns `None` when it was not
    /// registered, which changes nothing.
    pub fn remove_listener(
        &mut self,
        target: EventTargetId,
        event: &str,
        callback: u32,
    ) -> Option<Released> {
        if !self.listeners.remove(target, event, callback) {
            return None;
        }
        let touched = match target {
            EventTargetId::Signal(signal) => Some(signal),
            _ => None,
        };
        Some(self.released(vec![callback], touched))
    }

    /// Drops every registration on each of `nodes`.
    pub fn release_listeners(&mut self, nodes: &[NodeId]) -> Released {
        let callbacks = self.listeners.release_nodes(nodes);
        self.released(callbacks, [])
    }

    /// A new signal that is not aborted, `None` after the last id.
    pub fn create_signal(&mut self) -> Option<u32> {
        self.signals.create(false)
    }

    /// A new signal that is aborted already, `None` after the last id.
    pub fn create_aborted_signal(&mut self) -> Option<u32> {
        let id = self.signals.create(false)?;
        self.signals.mark_aborted(id);
        Some(id)
    }

    /// The signal of `AbortSignal.any(sources)`: aborted when a source is aborted, else dependent
    /// on `sources`. `None` after the last id.
    pub fn create_any_signal(&mut self, sources: &[u32]) -> Option<AnySignal> {
        if let Some(index) = sources.iter().position(|&s| self.signals.is_aborted(s)) {
            let id = self.create_aborted_signal()?;
            return Some(AnySignal {
                id,
                aborted_by: Some(index),
            });
        }
        let id = self.signals.create(true)?;
        self.signals.link_dependent(id, sources);
        Some(AnySignal {
            id,
            aborted_by: None,
        })
    }

    /// Marks `id` aborted with its dependents. Returns the dependents it aborted, `None` when `id`
    /// is aborted already or unknown. The abort steps of each signal follow with
    /// [`Self::remove_abort_registrations`].
    pub fn abort_signal(&mut self, id: u32) -> Option<Vec<u32>> {
        self.signals.mark_aborted(id)
    }

    /// The removal of the abort steps of `id`: drops the registrations that name `id` as their
    /// `signal` option. The signal table holds `id` until its event has run, then stops.
    pub fn remove_abort_registrations(&mut self, id: u32) -> Released {
        let mut callbacks = Vec::new();
        let mut touched = Vec::new();
        for (callback, link) in self.listeners.links.take(id) {
            if self.listeners.remove(link.target, &link.event, callback) {
                callbacks.push(callback);
                if let EventTargetId::Signal(target) = link.target {
                    touched.push(target);
                }
            }
        }
        self.released(callbacks, touched)
    }

    /// Drops everything of the collected signal `id`: its state, its links and the registrations
    /// on it.
    pub fn release_signal(&mut self, id: u32) -> Released {
        let orphaned = self.signals.release(id);
        self.listeners.links.forget(id);
        let callbacks = self.listeners.release_target(EventTargetId::Signal(id));
        self.released(callbacks, orphaned)
    }

    /// `callbacks` with the signals to unpin: of `touched` and of the signals that lost a link,
    /// those that nothing keeps pinned.
    pub(crate) fn released(
        &mut self,
        callbacks: Vec<u32>,
        touched: impl IntoIterator<Item = u32>,
    ) -> Released {
        let mut candidates: Vec<u32> = touched.into_iter().collect();
        candidates.extend(self.listeners.links.take_freed());
        let unpin = candidates
            .into_iter()
            .filter(|&id| !self.stays_pinned(id))
            .collect();
        Released { callbacks, unpin }
    }

    /// Whether the signal table keeps holding `id`: a signal that pins while an `abort` listener
    /// or a linked registration is left on it.
    fn stays_pinned(&self, id: u32) -> bool {
        let listened = self
            .listeners
            .has_listener(EventTargetId::Signal(id), "abort")
            || self.listeners.links().has(id);
        self.signals.pins(id) && listened
    }

    /// Registers `callback` for `event` on `target` with no option and no signal.
    #[cfg(any(test, feature = "test-support"))]
    pub fn add_plain_listener(
        &mut self,
        target: EventTargetId,
        event: &str,
        callback: u32,
    ) -> Added {
        let added = self.add_listener(target, event.to_owned(), false, false, None, || {
            Ok::<_, std::convert::Infallible>(callback)
        });
        match added {
            Ok(Some(added)) => added,
            Ok(None) => unreachable!("a listener with no signal is always added"),
            Err(never) => match never {},
        }
    }

    /// Drops every registration through [`Self::remove_listener`].
    #[cfg(any(test, feature = "test-support"))]
    pub fn remove_every_listener(&mut self) -> Released {
        let mut all = Released::default();
        for (target, event, callback) in self.listeners.registrations() {
            if let Some(released) = self.remove_listener(target, &event, callback) {
                all.callbacks.extend(released.callbacks);
                all.unpin.extend(released.unpin);
            }
        }
        all
    }

    /// Makes `next` the id of the next signal, so a test reaches the last id.
    #[cfg(any(test, feature = "test-support"))]
    pub fn start_signal_ids_at(&mut self, next: u32) {
        self.signals = Signals::starting_at(next);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::convert::Infallible;

    use super::*;

    const NODE: EventTargetId = EventTargetId::Node(5);

    /// Adds a `click` listener on [`NODE`] with `signal`, its callback id `callback`.
    fn add(state: &mut DocumentState, callback: u32, signal: Option<u32>) -> Option<Added> {
        state
            .add_listener(NODE, "click".to_owned(), false, false, signal, || {
                Ok::<_, Infallible>(callback)
            })
            .unwrap()
    }

    /// A document with a source signal and a signal that depends on it.
    fn dependent_pair() -> (DocumentState, u32, u32) {
        let mut state = DocumentState::default();
        let source = state.create_signal().unwrap();
        let dependent = state.create_any_signal(&[source]).unwrap().id;
        (state, source, dependent)
    }

    #[test]
    fn a_listener_with_an_aborted_signal_is_refused_before_its_callback_is_made() {
        let mut state = DocumentState::default();
        let signal = state.create_aborted_signal().unwrap();

        let added = state
            .add_listener(NODE, "click".to_owned(), false, false, Some(signal), || {
                Err::<u32, &str>("called")
            })
            .unwrap();

        assert_eq!(added, None);
        assert!(state.listeners().is_empty());
    }

    #[test]
    fn a_failed_callback_registers_nothing() {
        let mut state = DocumentState::default();
        let signal = state.create_signal().unwrap();

        let added =
            state.add_listener(NODE, "click".to_owned(), false, false, Some(signal), || {
                Err::<u32, &str>("full")
            });

        assert_eq!(added, Err("full"));
        assert!(state.listeners().is_empty());
        assert!(state.listeners().links().is_empty());
    }

    #[test]
    fn a_listener_keeps_its_options_and_links_to_its_signal() {
        let mut state = DocumentState::default();
        let signal = state.create_signal().unwrap();

        state
            .add_listener(NODE, "click".to_owned(), true, true, Some(signal), || {
                Ok::<_, Infallible>(7)
            })
            .unwrap();

        assert_eq!(
            state.listeners().listeners_for(NODE, "click"),
            [crate::Listener {
                callback: 7,
                capture: true,
                passive: true
            }]
        );
        assert!(state.listeners().links().has(signal));
    }

    #[rstest::rstest]
    #[case::abort_on_a_dependent("abort", true)]
    #[case::other_event_on_a_dependent("click", false)]
    fn an_abort_listener_pins_the_dependent_it_listens_on(#[case] event: &str, #[case] pins: bool) {
        let (mut state, _, dependent) = dependent_pair();

        let added = state.add_plain_listener(EventTargetId::Signal(dependent), event, 1);

        assert_eq!(added.pin, if pins { vec![dependent] } else { vec![] });
    }

    #[test]
    fn an_abort_listener_on_a_signal_that_depends_on_nothing_pins_nothing() {
        let mut state = DocumentState::default();
        let signal = state.create_signal().unwrap();

        let added = state.add_plain_listener(EventTargetId::Signal(signal), "abort", 1);

        assert!(added.pin.is_empty());
    }

    #[rstest::rstest]
    #[case::dependent(true)]
    #[case::plain(false)]
    fn a_listener_pins_its_signal_option_while_it_is_a_dependent(#[case] dependent: bool) {
        let (mut state, source, any) = dependent_pair();
        let signal = if dependent { any } else { source };

        let added = add(&mut state, 1, Some(signal)).unwrap();

        assert_eq!(added.pin, if dependent { vec![any] } else { vec![] });
    }

    #[test]
    fn removing_an_unregistered_listener_changes_nothing() {
        let mut state = DocumentState::default();
        add(&mut state, 1, None);

        assert_eq!(state.remove_listener(NODE, "click", 2), None);
        assert_eq!(state.listeners().listeners_for(NODE, "click").len(), 1);
    }

    #[test]
    fn removing_the_last_linked_listener_unpins_its_dependent_signal() {
        let (mut state, _, dependent) = dependent_pair();
        add(&mut state, 1, Some(dependent));
        add(&mut state, 2, Some(dependent));

        let first = state.remove_listener(NODE, "click", 1).unwrap();
        let second = state.remove_listener(NODE, "click", 2).unwrap();

        assert_eq!(first.callbacks, [1]);
        assert!(first.unpin.is_empty());
        assert_eq!(second.unpin, [dependent]);
    }

    #[test]
    fn removing_the_last_abort_listener_unpins_the_signal_it_listened_on() {
        let (mut state, _, dependent) = dependent_pair();
        let target = EventTargetId::Signal(dependent);
        state.add_plain_listener(target, "abort", 1);
        state.add_plain_listener(target, "abort", 2);

        let first = state.remove_listener(target, "abort", 1).unwrap();
        let second = state.remove_listener(target, "abort", 2).unwrap();

        assert!(first.unpin.is_empty());
        assert_eq!(second.unpin, [dependent]);
    }

    #[test]
    fn releasing_the_listeners_of_nodes_unpins_the_signal_they_linked() {
        let (mut state, _, dependent) = dependent_pair();
        add(&mut state, 1, Some(dependent));

        let released = state.release_listeners(&[5]);

        assert_eq!(released.callbacks, [1]);
        assert_eq!(released.unpin, [dependent]);
        assert!(state.listeners().links().is_empty());
    }

    #[test]
    fn any_of_an_aborted_source_is_aborted_and_depends_on_nothing() {
        let mut state = DocumentState::default();
        let live = state.create_signal().unwrap();
        let aborted = state.create_aborted_signal().unwrap();

        let any = state.create_any_signal(&[live, aborted]).unwrap();

        assert_eq!(any.aborted_by, Some(1));
        assert!(state.signals().is_aborted(any.id));
        assert!(!state.signals().is_dependent(any.id));
    }

    #[test]
    fn the_abort_removal_drops_the_linked_registrations_and_keeps_the_others() {
        let mut state = DocumentState::default();
        let signal = state.create_signal().unwrap();
        add(&mut state, 1, Some(signal));
        add(&mut state, 2, None);
        state.abort_signal(signal);

        let released = state.remove_abort_registrations(signal);

        assert_eq!(released.callbacks, [1]);
        assert_eq!(state.listeners().listeners_for(NODE, "click").len(), 1);
        assert!(state.listeners().links().is_empty());
    }

    #[test]
    fn the_abort_removal_unpins_a_signal_whose_last_abort_listener_it_dropped() {
        let (mut state, _, dependent) = dependent_pair();
        let signal = state.create_signal().unwrap();
        state
            .add_listener(
                EventTargetId::Signal(dependent),
                "abort".to_owned(),
                false,
                false,
                Some(signal),
                || Ok::<_, Infallible>(1),
            )
            .unwrap();
        state.abort_signal(signal);

        let released = state.remove_abort_registrations(signal);

        assert_eq!(released.unpin, [dependent]);
    }

    #[test]
    fn a_released_signal_drops_its_registrations_and_unpins_its_orphaned_dependents() {
        let (mut state, source, dependent) = dependent_pair();
        state.add_plain_listener(EventTargetId::Signal(source), "abort", 1);
        add(&mut state, 2, Some(source));
        add(&mut state, 3, Some(dependent));

        let released = state.release_signal(source);

        assert_eq!(released.callbacks, [1]);
        assert_eq!(released.unpin, [dependent]);
        assert!(!state.signals().contains(source));
        assert!(!state.listeners().links().has(source));
        assert_eq!(state.listeners().listeners_for(NODE, "click").len(), 2);
    }
}
