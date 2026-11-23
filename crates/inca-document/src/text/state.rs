// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The text, caret, selection and composition of an `input` node, in byte offsets of its value on
//! char boundaries, with the conversions to the UTF-16 code units of the platform's text input.

use std::ops::Range;

use inca_node::input_text::sanitized;
use unicode_segmentation::UnicodeSegmentation;

/// A caret move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Back,
    Forward,
    Start,
    End,
}

/// The text state of one `input` node.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InputState {
    pub(super) value: String,
    pub(super) selection: Range<usize>,
    pub(super) reversed: bool,
    pub(super) composition: Option<Range<usize>>,
    /// The value the field had when it took focus or at its last `change`;
    /// `None` while unfocused.
    pub(super) committed: Option<String>,
    /// An edit changed the field since `committed` was set.
    pub(super) edited: bool,
}

/// The first char boundary of `text` whose UTF-16 count reaches `units`. A
/// unit inside a surrogate pair rounds up, and a count past the end gives
/// the length.
pub(super) fn bytes_of_units(text: &str, units: usize) -> usize {
    let mut count = 0;
    for (at, c) in text.char_indices() {
        if count >= units {
            return at;
        }
        count += c.len_utf16();
    }
    text.len()
}

impl InputState {
    /// The text.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// The selected bytes of the text.
    #[must_use]
    pub fn selection(&self) -> Range<usize> {
        self.selection.clone()
    }

    /// Whether the caret sits at the start of the selection.
    #[must_use]
    pub fn reversed(&self) -> bool {
        self.reversed
    }

    /// The end of the selection that moves.
    #[must_use]
    pub fn caret(&self) -> usize {
        if self.reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }

    /// The end of the selection that stays.
    #[must_use]
    pub fn anchor(&self) -> usize {
        if self.reversed {
            self.selection.end
        } else {
            self.selection.start
        }
    }

    /// The composed bytes of the text.
    #[must_use]
    pub fn composition(&self) -> Option<Range<usize>> {
        self.composition.clone()
    }

    /// The selected text.
    #[must_use]
    pub fn selected_text(&self) -> &str {
        &self.value[self.selection.clone()]
    }

    /// The nearest char boundary at or below `offset`, at most the length.
    pub(super) fn clamp(&self, offset: usize) -> usize {
        self.value.floor_char_boundary(offset)
    }

    /// Replaces the text with `value` without CR and LF. A changed text puts
    /// the caret at its end and ends the composition. Returns whether the
    /// text changed.
    pub fn set_value(&mut self, value: &str) -> bool {
        let value = sanitized(value);
        if value == self.value {
            return false;
        }
        let end = value.len();
        self.value = value;
        self.selection = end..end;
        self.reversed = false;
        self.composition = None;
        true
    }

    /// Selects from `anchor` to `focus`. The caret sits at `focus`.
    pub fn select(&mut self, anchor: usize, focus: usize) {
        let (anchor, focus) = (self.clamp(anchor), self.clamp(focus));
        self.selection = anchor.min(focus)..anchor.max(focus);
        self.reversed = focus < anchor;
    }

    /// Selects the whole text.
    pub fn select_all(&mut self) {
        self.select(0, self.value.len());
    }

    /// The start of the grapheme after `from` going `forward`, else before
    /// it, else the end of the text or 0.
    #[must_use]
    pub fn step(&self, from: usize, forward: bool) -> usize {
        let mut starts = self.value.grapheme_indices(true).map(|(at, _)| at);
        if forward {
            starts.find(|&at| at > from).unwrap_or(self.value.len())
        } else {
            starts.take_while(|&at| at < from).last().unwrap_or(0)
        }
    }

    /// Moves the caret, and the selection end with it while `extend` holds.
    /// `macos` makes Shift+Home and Shift+End extend the selection to the
    /// target and keep its far end.
    pub fn motion(&mut self, motion: Motion, extend: bool, macos: bool) {
        let caret = self.caret();
        let collapses = !extend && !self.selection.is_empty();
        let target = match motion {
            Motion::Back if collapses => self.selection.start,
            Motion::Back => self.step(caret, false),
            Motion::Forward if collapses => self.selection.end,
            Motion::Forward => self.step(caret, true),
            Motion::Start => 0,
            Motion::End => self.value.len(),
        };
        if !extend {
            self.select(target, target);
        } else if macos && matches!(motion, Motion::Start | Motion::End) {
            let reversed = if self.selection.is_empty() {
                target < caret
            } else {
                self.reversed
            };
            self.selection = self.selection.start.min(target)..self.selection.end.max(target);
            self.reversed = reversed;
        } else {
            self.select(self.anchor(), target);
        }
    }

    /// Puts the caret at `offset`, and keeps the anchor while `extend` holds.
    pub fn place(&mut self, offset: usize, extend: bool) {
        if extend {
            self.select(self.anchor(), offset);
        } else {
            self.select(offset, offset);
        }
    }

    /// The UTF-16 offset of the byte `offset`.
    #[must_use]
    pub fn to_utf16(&self, offset: usize) -> usize {
        self.value[..self.clamp(offset)].encode_utf16().count()
    }

    /// The byte offset of the UTF-16 offset `units`.
    #[must_use]
    pub fn from_utf16(&self, units: usize) -> usize {
        bytes_of_units(&self.value, units)
    }

    /// `range` in UTF-16 offsets.
    #[must_use]
    pub fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.to_utf16(range.start)..self.to_utf16(range.end)
    }

    /// `range` in byte offsets.
    #[must_use]
    pub fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.from_utf16(range.start)..self.from_utf16(range.end)
    }

    /// The length of the text in UTF-16 code units.
    #[must_use]
    pub fn len_utf16(&self) -> usize {
        self.value.encode_utf16().count()
    }

    /// The composition, else the selection.
    pub(super) fn target(&self) -> Range<usize> {
        self.composition
            .clone()
            .unwrap_or_else(|| self.selection.clone())
    }

    /// Whether an edit changed the text since the baseline was set.
    pub(super) fn changed(&self) -> bool {
        self.edited && self.committed.as_deref().is_some_and(|c| c != self.value)
    }

    /// Records the text as the baseline of the next `change`.
    pub fn begin_focus(&mut self) {
        self.committed = Some(self.value.clone());
        self.edited = false;
    }

    /// Drops the baseline. Returns whether a `change` is due: an edit since
    /// the baseline left a different text.
    pub fn end_focus(&mut self) -> bool {
        let changed = self.changed();
        self.committed = None;
        self.edited = false;
        changed
    }

    /// Returns whether a `change` is due, as [`end_focus`](Self::end_focus)
    /// does, and moves a set baseline to the text.
    pub fn commit(&mut self) -> bool {
        let changed = self.changed();
        if self.committed.is_some() {
            self.committed = Some(self.value.clone());
            self.edited = false;
        }
        changed
    }

    /// Ends the composition and keeps its text.
    pub fn unmark(&mut self) {
        self.composition = None;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;
    use crate::InputType;
    use crate::tests::{at, caret_at};
    use rstest::rstest;

    /// A focused state holding `value`.
    fn focused(value: &str) -> InputState {
        let mut state = caret_at(value, value.len());
        state.begin_focus();
        state
    }

    #[test]
    fn set_value_strips_line_breaks() {
        let mut state = InputState::default();
        state.set_value("a\nb");
        assert_eq!(state.value(), "ab");
    }

    #[test]
    fn set_value_puts_the_caret_at_the_end_of_a_new_value() {
        let mut state = at("xy", 0..1, true);
        state.composition = Some(0..1);
        assert!(state.set_value("abc"));
        assert_eq!(state.selection(), 3..3);
        assert!(!state.reversed());
        assert_eq!(state.composition(), None);
    }

    #[test]
    fn set_value_keeps_the_selection_for_the_same_value() {
        let mut state = at("abc", 0..2, true);
        assert!(!state.set_value("abc"));
        assert_eq!(state.selection(), 0..2);
        assert!(state.reversed());
    }

    #[rstest]
    #[case::forward("abcd", 1, 3, 1..3, false)]
    #[case::backward("abcd", 3, 1, 1..3, true)]
    #[case::clamped("abcd", 1, 99, 1..4, false)]
    #[case::inside_a_char("é", 1, 1, 0..0, false)]
    fn select_orders_the_ends_by_direction(
        #[case] value: &str,
        #[case] anchor: usize,
        #[case] focus: usize,
        #[case] want: Range<usize>,
        #[case] reversed: bool,
    ) {
        let mut state = caret_at(value, 0);
        state.select(anchor, focus);
        assert_eq!(state.selection(), want);
        assert_eq!(state.reversed(), reversed);
    }

    #[rstest]
    #[case::back(2..2, false, Motion::Back, false, false, 1..1, false)]
    #[case::back_collapses(1..3, false, Motion::Back, false, false, 1..1, false)]
    #[case::forward_collapses(1..3, false, Motion::Forward, false, false, 3..3, false)]
    #[case::forward(2..2, false, Motion::Forward, false, false, 3..3, false)]
    #[case::back_at_start(0..0, false, Motion::Back, false, false, 0..0, false)]
    #[case::forward_at_end(4..4, false, Motion::Forward, false, false, 4..4, false)]
    #[case::start(2..2, false, Motion::Start, false, false, 0..0, false)]
    #[case::end(2..2, false, Motion::End, false, false, 4..4, false)]
    #[case::extend_back(2..2, false, Motion::Back, true, false, 1..2, true)]
    #[case::extend_forward_from_a_reversed_selection(
        1..2, true, Motion::Forward, true, false, 2..2, false
    )]
    #[case::extend_start(2..2, false, Motion::Start, true, false, 0..2, true)]
    #[case::extend_end(2..2, false, Motion::End, true, false, 2..4, false)]
    #[case::linux_end_after_home(0..2, true, Motion::End, true, false, 2..4, false)]
    #[case::mac_home_from_caret(2..2, false, Motion::Start, true, true, 0..2, true)]
    #[case::mac_end_after_home(0..2, true, Motion::End, true, true, 0..4, true)]
    #[case::mac_end_from_caret(2..2, false, Motion::End, true, true, 2..4, false)]
    fn a_motion_moves_or_extends_the_caret(
        #[case] selection: Range<usize>,
        #[case] reversed: bool,
        #[case] motion: Motion,
        #[case] extend: bool,
        #[case] macos: bool,
        #[case] want: Range<usize>,
        #[case] want_reversed: bool,
    ) {
        let mut state = at("abcd", selection, reversed);
        state.motion(motion, extend, macos);
        assert_eq!(state.selection(), want);
        assert_eq!(state.reversed(), want_reversed);
    }

    #[rstest]
    #[case::combining("e\u{301}x", 3, false, 0)]
    #[case::emoji_forward("😀a", 0, true, 4)]
    fn a_step_crosses_one_grapheme(
        #[case] value: &str,
        #[case] from: usize,
        #[case] forward: bool,
        #[case] want: usize,
    ) {
        assert_eq!(caret_at(value, 0).step(from, forward), want);
    }

    #[rstest]
    #[case::moves(1..3, 2, false, 2..2)]
    #[case::extend(1..1, 3, true, 1..3)]
    fn place_moves_or_extends_from_the_anchor(
        #[case] selection: Range<usize>,
        #[case] offset: usize,
        #[case] extend: bool,
        #[case] want: Range<usize>,
    ) {
        let mut state = at("abcd", selection, false);
        state.place(offset, extend);
        assert_eq!(state.selection(), want);
    }

    #[test]
    fn select_all_selects_the_whole_value() {
        let mut state = caret_at("abc", 1);
        state.select_all();
        assert_eq!(state.selection(), 0..3);
    }

    #[test]
    fn to_utf16_counts_the_units_before_a_byte_offset() {
        assert_eq!(caret_at("a😀b", 0).to_utf16(5), 3);
    }

    #[rstest]
    #[case::from(3, 5)]
    #[case::from_inside_a_pair(2, 5)]
    #[case::from_past_the_end(9, 6)]
    fn from_utf16_gives_the_byte_offset_of_a_unit_offset(
        #[case] units: usize,
        #[case] want: usize,
    ) {
        assert_eq!(caret_at("a😀b", 0).from_utf16(units), want);
    }

    #[test]
    fn unmark_keeps_the_text() {
        let mut state = caret_at("abk", 3);
        state.composition = Some(2..3);
        state.unmark();
        assert_eq!(state.composition(), None);
        assert_eq!(state.value(), "abk");
    }

    #[test]
    fn end_focus_reports_an_edited_changed_value() {
        let mut state = focused("ab");
        state.apply(&state.insert("x", InputType::InsertText).unwrap());
        assert!(state.end_focus());
    }

    #[test]
    fn end_focus_reports_nothing_for_a_value_edited_back() {
        let mut state = focused("ab");
        state.apply(&state.insert("x", InputType::InsertText).unwrap());
        state.apply(&state.delete(false));
        assert!(!state.end_focus());
    }

    #[test]
    fn end_focus_reports_nothing_without_an_edit() {
        assert!(!focused("ab").end_focus());
    }

    #[test]
    fn end_focus_of_an_unfocused_state_reports_nothing() {
        let mut state = caret_at("ab", 2);
        state.apply(&state.insert("x", InputType::InsertText).unwrap());
        assert!(!state.end_focus());
    }

    #[test]
    fn commit_moves_the_change_baseline() {
        let mut state = focused("ab");
        state.apply(&state.insert("x", InputType::InsertText).unwrap());
        assert!(state.commit());
        assert!(!state.commit());
    }

    #[test]
    fn commit_of_an_unfocused_state_keeps_no_baseline() {
        let mut state = caret_at("ab", 2);
        state.apply(&state.insert("x", InputType::InsertText).unwrap());
        assert!(!state.commit());
        assert_eq!(state.committed, None);
    }

    #[test]
    fn set_value_after_an_edit_leaves_a_change() {
        let mut state = focused("ab");
        state.apply(&state.insert("x", InputType::InsertText).unwrap());
        state.set_value("z");
        assert!(state.end_focus());
    }

    #[test]
    fn set_value_alone_leaves_no_change() {
        let mut state = focused("ab");
        state.set_value("z");
        assert!(!state.end_focus());
    }
}
