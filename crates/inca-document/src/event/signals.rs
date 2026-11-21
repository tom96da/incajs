// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The state of the `AbortSignal`s and the links from a signal to the registrations its abort
//! removes.

use std::collections::{BTreeSet, HashMap};

use super::EventTargetId;

/// The state of one signal.
#[derive(Debug)]
struct State {
    aborted: bool,
    dependent: bool,
    sources: Vec<u32>,
    dependents: Vec<u32>,
}

/// The state records of the signals, by id.
#[derive(Debug)]
pub struct Signals {
    next: Option<u32>,
    by_id: HashMap<u32, State>,
}

impl Default for Signals {
    fn default() -> Self {
        Self::starting_at(1)
    }
}

impl Signals {
    /// An empty set whose first id is `next`.
    #[must_use]
    pub fn starting_at(next: u32) -> Self {
        Self {
            next: Some(next),
            by_id: HashMap::new(),
        }
    }

    /// A new record under the next id, `None` after the last id.
    pub fn create(&mut self, dependent: bool) -> Option<u32> {
        let id = self.next?;
        self.next = id.checked_add(1);
        self.by_id.insert(
            id,
            State {
                aborted: false,
                dependent,
                sources: Vec::new(),
                dependents: Vec::new(),
            },
        );
        Some(id)
    }

    /// Marks `id` aborted, and every dependent that is not aborted yet. Returns those dependents,
    /// `None` when `id` is aborted already or unknown.
    pub fn mark_aborted(&mut self, id: u32) -> Option<Vec<u32>> {
        let state = self.by_id.get_mut(&id).filter(|state| !state.aborted)?;
        state.aborted = true;
        let dependents = state.dependents.clone();
        let mut pending = Vec::new();
        for dependent in dependents {
            if let Some(state) = self.by_id.get_mut(&dependent).filter(|s| !s.aborted) {
                state.aborted = true;
                pending.push(dependent);
            }
        }
        Some(pending)
    }

    /// Makes `result` depend on `sources`. A source that is dependent contributes its own
    /// sources, and each source counts once.
    pub fn link_dependent(&mut self, result: u32, sources: &[u32]) {
        let mut flat: Vec<u32> = Vec::new();
        for source in sources {
            let own = self
                .by_id
                .get(source)
                .map(|s| (s.dependent, s.sources.clone()));
            let (dependent, inherited) = own.unwrap_or_default();
            let given = if dependent { inherited } else { vec![*source] };
            for id in given {
                if !flat.contains(&id) {
                    flat.push(id);
                }
            }
        }
        for source in &flat {
            if let Some(state) = self.by_id.get_mut(source) {
                state.dependents.push(result);
            }
        }
        if let Some(state) = self.by_id.get_mut(&result) {
            state.sources = flat;
        }
    }

    /// Drops the record of `id` and its links. Returns the dependents that have no source left.
    pub fn release(&mut self, id: u32) -> Vec<u32> {
        let Some(state) = self.by_id.remove(&id) else {
            return Vec::new();
        };
        for source in state.sources {
            if let Some(source) = self.by_id.get_mut(&source) {
                source.dependents.retain(|d| *d != id);
            }
        }
        let mut orphaned = Vec::new();
        for dependent in state.dependents {
            if let Some(dependent_state) = self.by_id.get_mut(&dependent) {
                dependent_state.sources.retain(|s| *s != id);
                if dependent_state.sources.is_empty() {
                    orphaned.push(dependent);
                }
            }
        }
        orphaned
    }

    /// Whether `id` has a record.
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn contains(&self, id: u32) -> bool {
        self.by_id.contains_key(&id)
    }

    #[must_use]
    pub fn is_aborted(&self, id: u32) -> bool {
        self.by_id.get(&id).is_some_and(|s| s.aborted)
    }

    #[must_use]
    pub fn is_dependent(&self, id: u32) -> bool {
        self.by_id.get(&id).is_some_and(|s| s.dependent)
    }

    fn has_source(&self, id: u32) -> bool {
        self.by_id.get(&id).is_some_and(|s| !s.sources.is_empty())
    }

    /// Whether an `abort` listener on `id` keeps the signal alive. That holds for a dependent that
    /// is not aborted and has a source left.
    #[must_use]
    pub fn pins(&self, id: u32) -> bool {
        self.is_dependent(id) && !self.is_aborted(id) && self.has_source(id)
    }
}

/// Where a linked registration sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub target: EventTargetId,
    pub event: String,
}

/// For each signal the registrations that name it as their `signal` option, and for each such
/// registration the signal. Plain ids and names, no JS value.
#[derive(Debug, Default)]
pub struct Links {
    by_callback: HashMap<u32, (u32, Link)>,
    by_signal: HashMap<u32, BTreeSet<u32>>,
    freed: Vec<u32>,
}

impl Links {
    /// Records that the abort of `signal` removes the registration with `callback`.
    pub fn link(&mut self, signal: u32, callback: u32, link: Link) {
        self.by_callback.insert(callback, (signal, link));
        self.by_signal.entry(signal).or_default().insert(callback);
    }

    /// Drops the link of `callback` and records its signal as freed. Returns the signal.
    pub fn unlink(&mut self, callback: u32) -> Option<u32> {
        let (signal, _) = self.by_callback.remove(&callback)?;
        self.drop_from_signal(signal, callback);
        self.freed.push(signal);
        Some(signal)
    }

    /// The callback id and link of each registration of `signal`, removed from both maps, in
    /// callback id order.
    pub fn take(&mut self, signal: u32) -> Vec<(u32, Link)> {
        let callbacks = self.by_signal.remove(&signal).unwrap_or_default();
        callbacks
            .into_iter()
            .filter_map(|callback| {
                let (_, link) = self.by_callback.remove(&callback)?;
                Some((callback, link))
            })
            .collect()
    }

    /// Whether `signal` has a link left.
    #[must_use]
    pub fn has(&self, signal: u32) -> bool {
        self.by_signal.contains_key(&signal)
    }

    /// Drops the links of `signal` and leaves the registrations in place.
    pub fn forget(&mut self, signal: u32) {
        drop(self.take(signal));
    }

    /// The signals that lost a link since the last call.
    pub fn take_freed(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.freed)
    }

    fn drop_from_signal(&mut self, signal: u32, callback: u32) {
        if let Some(callbacks) = self.by_signal.get_mut(&signal) {
            callbacks.remove(&callback);
            if callbacks.is_empty() {
                self.by_signal.remove(&signal);
            }
        }
    }

    /// How many links exist.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_callback.len()
    }

    /// Whether no link exists.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_callback.is_empty()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn link(callback: u32) -> Link {
        Link {
            target: EventTargetId::Node(callback),
            event: "click".to_owned(),
        }
    }

    #[test]
    fn a_signal_gives_its_links_in_callback_order_and_loses_them() {
        let mut links = Links::default();
        links.link(1, 9, link(9));
        links.link(1, 4, link(4));
        links.link(2, 5, link(5));

        let taken: Vec<u32> = links.take(1).into_iter().map(|(c, _)| c).collect();

        assert_eq!(taken, [4, 9]);
        assert!(!links.has(1));
        assert!(links.has(2));
        assert_eq!(links.len(), 1);
    }

    #[test]
    fn unlinking_a_callback_gives_its_signal_and_records_it_as_freed() {
        let mut links = Links::default();
        links.link(1, 4, link(4));

        assert_eq!(links.unlink(4), Some(1));
        assert_eq!(links.unlink(4), None);
        assert_eq!(links.take_freed(), [1]);
        assert!(links.take_freed().is_empty());
        assert!(!links.has(1));
    }

    #[test]
    fn a_signal_keeps_its_link_while_another_callback_is_unlinked() {
        let mut links = Links::default();
        links.link(1, 4, link(4));
        links.link(1, 5, link(5));

        links.unlink(4);

        assert!(links.has(1));
    }

    #[test]
    fn forgetting_a_signal_drops_its_links_without_a_freed_record() {
        let mut links = Links::default();
        links.link(1, 4, link(4));

        links.forget(1);

        assert_eq!(links.len(), 0);
        assert!(links.take_freed().is_empty());
    }

    #[test]
    fn signals_run_out_of_ids_after_the_last_one() {
        let mut signals = Signals::starting_at(u32::MAX);

        assert_eq!(signals.create(false), Some(u32::MAX));
        assert_eq!(signals.create(false), None);
    }

    #[test]
    fn a_source_that_appears_twice_is_linked_once() {
        let mut signals = Signals::default();
        let (a, b) = (
            signals.create(false).unwrap(),
            signals.create(true).unwrap(),
        );

        signals.link_dependent(b, &[a, a]);

        assert_eq!(signals.by_id[&a].dependents, [b]);
        assert_eq!(signals.by_id[&b].sources, [a]);
        assert_eq!(signals.mark_aborted(a), Some(vec![b]));
    }

    fn linked_pair() -> (Signals, u32, u32) {
        let mut signals = Signals::default();
        let a = signals.create(false).unwrap();
        let b = signals.create(true).unwrap();
        signals.link_dependent(b, &[a]);
        (signals, a, b)
    }

    #[test]
    fn aborting_a_source_aborts_its_dependents() {
        let (mut signals, a, b) = linked_pair();

        assert_eq!(signals.mark_aborted(a), Some(vec![b]));
        assert!(signals.is_aborted(a) && signals.is_aborted(b));
    }

    #[test]
    fn aborting_a_signal_twice_gives_no_dependents_the_second_time() {
        let (mut signals, a, _) = linked_pair();

        signals.mark_aborted(a);

        assert_eq!(signals.mark_aborted(a), None);
    }

    #[test]
    fn aborting_an_unknown_signal_gives_nothing() {
        let mut signals = Signals::default();

        assert_eq!(signals.mark_aborted(99), None);
    }

    #[test]
    fn a_dependent_signal_is_pinned_until_it_aborts() {
        let (mut signals, a, b) = linked_pair();

        assert!(signals.pins(b));
        signals.mark_aborted(a);
        assert!(!signals.pins(b));
    }

    #[test]
    fn only_the_signal_made_dependent_reads_as_dependent() {
        let (signals, a, b) = linked_pair();

        assert!(signals.is_dependent(b));
        assert!(!signals.is_dependent(a));
    }

    #[test]
    fn releasing_a_source_orphans_its_dependents() {
        let mut signals = Signals::default();
        let a = signals.create(false).unwrap();
        let b = signals.create(true).unwrap();
        signals.link_dependent(b, &[a]);

        assert_eq!(signals.release(a), vec![b]);
        assert!(!signals.contains(a));
        assert!(!signals.pins(b));
        assert_eq!(signals.release(a), Vec::<u32>::new());
    }
}
