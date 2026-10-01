// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What `console` remembers between calls: counters, timers and group depth.

use std::collections::HashMap;
use std::time::Instant;

/// Counters, running timers and the number of open groups.
#[derive(Debug, Default)]
pub(super) struct State {
    counts: HashMap<String, u64>,
    timers: HashMap<String, Instant>,
    depth: usize,
}

impl State {
    /// Open groups, which is how many guides a line starts with.
    pub(super) fn depth(&self) -> usize {
        self.depth
    }

    pub(super) fn open_group(&mut self) {
        self.depth += 1;
    }

    /// Closes the innermost group; does nothing when none is open.
    pub(super) fn close_group(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    pub(super) fn close_all_groups(&mut self) {
        self.depth = 0;
    }

    /// Adds one to `label`'s count and returns the new value.
    pub(super) fn count(&mut self, label: &str) -> u64 {
        let count = self.counts.entry(label.to_owned()).or_insert(0);
        *count += 1;
        *count
    }

    /// Zeroes `label`'s count; `false` when it was never counted.
    pub(super) fn reset_count(&mut self, label: &str) -> bool {
        match self.counts.get_mut(label) {
            Some(count) => {
                *count = 0;
                true
            }
            None => false,
        }
    }

    /// Starts `label`'s timer; `false` when one is already running.
    pub(super) fn start_timer(&mut self, label: &str) -> bool {
        self.start_timer_at(label, Instant::now())
    }

    fn start_timer_at(&mut self, label: &str, at: Instant) -> bool {
        if self.timers.contains_key(label) {
            return false;
        }
        self.timers.insert(label.to_owned(), at);
        true
    }

    /// Milliseconds since `label`'s timer started, if it is running.
    pub(super) fn elapsed_ms(&self, label: &str) -> Option<f64> {
        self.timers
            .get(label)
            .map(|start| start.elapsed().as_secs_f64() * 1000.0)
    }

    /// Stops `label`'s timer and returns its elapsed milliseconds.
    pub(super) fn end_timer(&mut self, label: &str) -> Option<f64> {
        let elapsed = self.elapsed_ms(label)?;
        self.timers.remove(label);
        Some(elapsed)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn counts_run_per_label() {
        let mut state = State::default();

        assert_eq!(state.count("a"), 1);
        assert_eq!(state.count("a"), 2);
        assert_eq!(state.count("b"), 1);
    }

    #[test]
    fn reset_count_zeroes_a_known_label_only() {
        let mut state = State::default();
        state.count("a");

        assert!(state.reset_count("a"));
        assert_eq!(state.count("a"), 1);
        assert!(!state.reset_count("missing"));
    }

    #[test]
    fn a_second_start_is_refused_and_keeps_the_first() {
        let mut state = State::default();
        let long_ago = Instant::now().checked_sub(Duration::from_secs(10)).unwrap();

        assert!(state.start_timer_at("t", long_ago));
        assert!(!state.start_timer("t"));
        assert!(state.elapsed_ms("t").unwrap() >= 10_000.0);
    }

    #[test]
    fn ending_a_timer_removes_it() {
        let mut state = State::default();
        state.start_timer("t");

        assert!(state.end_timer("t").is_some());
        assert!(state.elapsed_ms("t").is_none());
        assert!(state.end_timer("t").is_none());
        assert!(state.start_timer("t"), "the label is free again");
    }

    #[test]
    fn an_unknown_timer_has_no_elapsed_time() {
        assert!(State::default().elapsed_ms("x").is_none());
    }

    #[test]
    fn groups_nest_and_underflow_stays_at_zero() {
        let mut state = State::default();

        state.close_group();
        assert_eq!(state.depth(), 0);
        state.open_group();
        state.open_group();
        assert_eq!(state.depth(), 2);
        state.close_group();
        assert_eq!(state.depth(), 1);
        state.close_all_groups();
        assert_eq!(state.depth(), 0);
    }
}
