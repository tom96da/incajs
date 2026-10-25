// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::float_cmp, clippy::unwrap_used)]

use rstest::rstest;

use super::*;

/// A state holding `value` with `selection` selected.
fn at(value: &str, selection: Range<usize>, reversed: bool) -> InputState {
    InputState {
        value: value.to_owned(),
        selection,
        reversed,
        ..InputState::default()
    }
}

/// A state with a caret at `caret` in `value`.
fn caret_at(value: &str, caret: usize) -> InputState {
    at(value, caret..caret, false)
}

/// A state that recorded a line starting at window x `x` with `stops`.
fn laid_out(x: f32, stops: &[(usize, f32)]) -> InputState {
    let mut state = InputState::default();
    state.record(InputLayout {
        x,
        y: 20.0,
        height: 16.0,
        scroll: 0.0,
        stops: stops.to_vec(),
    });
    state
}

#[test]
fn typed_text_drops_control_characters() {
    assert_eq!(typed("a\tb\nc\u{8}"), "abc");
}

#[test]
fn sanitized_text_drops_line_breaks_only() {
    assert_eq!(sanitized("a\r\nb\tc"), "ab\tc");
}

#[rstest]
#[case::lf("a\nb\tc", "a b\tc")]
#[case::crlf("a\r\nb", "a b")]
#[case::cr("a\rb", "a b")]
fn pasted_text_turns_each_line_break_into_a_space(#[case] text: &str, #[case] want: &str) {
    assert_eq!(pasted(text), want);
}

#[rstest]
#[case::exact(&[(0, 0.0), (3, 12.0), (6, 24.0)], 3, 12.0)]
#[case::between(&[(0, 0.0), (3, 12.0), (6, 24.0)], 4, 12.0)]
#[case::empty(&[], 3, 0.0)]
fn x_at_reads_the_stop_of_an_offset(
    #[case] stops: &[(usize, f32)],
    #[case] offset: usize,
    #[case] want: f32,
) {
    assert_eq!(x_at(stops, offset), want);
}

#[rstest]
#[case::ascii("ab", &[0, 1, 2])]
#[case::wide("é😀", &[0, 2, 6])]
#[case::empty("", &[0])]
fn stops_of_lists_every_char_boundary(#[case] value: &str, #[case] offsets: &[usize]) {
    #[allow(clippy::cast_precision_loss)]
    let x_for = |at: usize| at as f32 * 2.0;
    let want: Vec<_> = offsets.iter().map(|&at| (at, x_for(at))).collect();
    assert_eq!(stops_of(value, x_for), want);
}

#[rstest]
#[case::inside(0.0, 10.0, 50.0, 30.0, 0.0)]
#[case::right(0.0, 40.0, 60.0, 30.0, 11.0)]
#[case::left(20.0, 5.0, 60.0, 30.0, 5.0)]
#[case::short_line(50.0, 10.0, 10.0, 30.0, 0.0)]
#[case::end(0.0, 60.0, 60.0, 30.0, 31.0)]
fn scroll_to_caret_keeps_the_caret_in_the_box(
    #[case] scroll: f32,
    #[case] caret: f32,
    #[case] line_width: f32,
    #[case] width: f32,
    #[case] want: f32,
) {
    assert_eq!(scroll_to_caret(scroll, caret, line_width, width), want);
}

#[rstest]
#[case::unfocused(0..2, 2, false, None, None)]
#[case::unfocused_caret(1..1, 1, false, None, None)]
#[case::caret(1..1, 1, true, None, Some(6.0))]
#[case::selection(0..2, 2, true, Some((0.0, 12.0)), None)]
fn highlights_show_only_while_focused(
    #[case] selection: Range<usize>,
    #[case] caret: usize,
    #[case] focused: bool,
    #[case] want_selection: Option<(f32, f32)>,
    #[case] want_caret: Option<f32>,
) {
    let stops = [(0, 0.0), (1, 6.0), (2, 12.0)];
    let want = Highlights {
        selection: want_selection,
        caret: want_caret,
    };
    assert_eq!(highlights(&stops, &selection, caret, focused), want);
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

#[rstest]
#[case::nearer_right(0.0, 13.0, 2)]
#[case::tie(0.0, 3.0, 0)]
#[case::left_of_text(0.0, -5.0, 0)]
#[case::right_of_text(0.0, 99.0, 3)]
#[case::scrolled(-6.0, 0.0, 1)]
fn offset_at_finds_the_nearest_stop(#[case] layout_x: f32, #[case] x: f32, #[case] want: usize) {
    let stops = [(0, 0.0), (1, 6.0), (2, 12.0), (3, 18.0)];
    assert_eq!(laid_out(layout_x, &stops).offset_at(x), want);
}

#[test]
fn offset_at_gives_0_without_a_layout() {
    assert_eq!(InputState::default().offset_at(13.0), 0);
}

#[test]
fn to_utf16_counts_the_units_before_a_byte_offset() {
    assert_eq!(caret_at("a😀b", 0).to_utf16(5), 3);
}

#[rstest]
#[case::from(3, 5)]
#[case::from_inside_a_pair(2, 5)]
#[case::from_past_the_end(9, 6)]
fn from_utf16_gives_the_byte_offset_of_a_unit_offset(#[case] units: usize, #[case] want: usize) {
    assert_eq!(caret_at("a😀b", 0).from_utf16(units), want);
}

#[test]
fn insert_replaces_the_selection() {
    let edit = at("abcd", 1..3, false)
        .insert("x", InputType::InsertText)
        .unwrap();
    let want = Edit {
        input_type: InputType::InsertText,
        range: 1..3,
        text: "x".to_owned(),
        selection: 2..2,
        composition: None,
    };
    assert_eq!(edit, want);
}

#[test]
fn insert_replaces_the_composition() {
    let mut state = caret_at("abcd", 3);
    state.composition = Some(1..3);
    let edit = state.insert("x", InputType::InsertFromPaste).unwrap();
    assert_eq!(edit.range, 1..3);
    assert_eq!(edit.input_type, InputType::InsertFromPaste);
}

#[test]
fn insert_of_nothing_over_nothing_is_no_edit() {
    assert_eq!(caret_at("ab", 1).insert("", InputType::InsertText), None);
}

#[test]
fn replace_converts_its_utf16_range() {
    let edit = caret_at("a😀b", 0).replace(Some(1..3), "x").unwrap();
    assert_eq!(edit.range, 1..5);
    assert_eq!(edit.selection, 2..2);
    assert_eq!(edit.input_type, InputType::InsertText);
}

#[test]
fn replace_while_composing_is_a_composition_edit() {
    let mut state = caret_at("abk", 3);
    state.composition = Some(2..3);
    let edit = state.replace(None, "か").unwrap();
    assert_eq!(edit.input_type, InputType::InsertCompositionText);
    assert_eq!(edit.range, 2..3);
    assert_eq!(edit.composition, None);
}

#[rstest]
#[case::default_caret("ab", 1..2, None, "k", None, (1..2, 2..2, Some(1..2)))]
#[case::selected("", 0..0, None, "か", Some(0..1), (0..0, 0..3, Some(0..3)))]
#[case::empty_text("ab", 1..2, None, "", None, (1..2, 1..1, None))]
#[case::explicit_range("a😀b", 0..0, Some(1..3), "k", None, (1..5, 2..2, Some(1..2)))]
fn compose_marks_the_new_text(
    #[case] value: &str,
    #[case] selection: Range<usize>,
    #[case] range: Option<Range<usize>>,
    #[case] text: &str,
    #[case] selected: Option<Range<usize>>,
    #[case] want: (Range<usize>, Range<usize>, Option<Range<usize>>),
) {
    let edit = at(value, selection, false).compose(range, text, selected);
    assert_eq!(edit.input_type, InputType::InsertCompositionText);
    assert_eq!(edit.text, text);
    assert_eq!((edit.range, edit.selection, edit.composition), want);
}

#[rstest]
#[case::back("abc", 2..2, false, 1..2, InputType::DeleteContentBackward)]
#[case::back_combining("e\u{301}x", 3..3, false, 1..3, InputType::DeleteContentBackward)]
#[case::forward_combining("e\u{301}x", 0..0, true, 0..3, InputType::DeleteContentForward)]
#[case::selection("abcd", 1..3, false, 1..3, InputType::DeleteContentBackward)]
fn delete_removes_the_range_of_its_direction(
    #[case] value: &str,
    #[case] selection: Range<usize>,
    #[case] forward: bool,
    #[case] want: Range<usize>,
    #[case] input_type: InputType,
) {
    let edit = at(value, selection, false).delete(forward);
    assert_eq!(edit.range, want);
    assert_eq!(edit.input_type, input_type);
    assert_eq!(edit.text, "");
    assert_eq!(edit.selection, want.start..want.start);
    assert_eq!(edit.composition, None);
}

#[rstest]
#[case::back_at_start(0, false, 0..0)]
#[case::forward_at_end(3, true, 3..3)]
fn delete_at_an_edge_is_an_empty_edit(
    #[case] caret: usize,
    #[case] forward: bool,
    #[case] want: Range<usize>,
) {
    assert_eq!(caret_at("abc", caret).delete(forward).range, want);
}

#[test]
fn cut_takes_the_selection() {
    let edit = at("abcd", 1..3, false).cut().unwrap();
    assert_eq!(edit.input_type, InputType::DeleteByCut);
    assert_eq!(edit.range, 1..3);
    assert_eq!(edit.selection, 1..1);
}

#[test]
fn cut_of_an_empty_selection_is_no_edit() {
    assert!(at("abcd", 2..2, false).cut().is_none());
}

#[test]
fn apply_writes_the_edit() {
    let mut state = at("abcd", 1..3, true);
    state.composition = Some(1..2);
    state.apply(&Edit {
        input_type: InputType::InsertCompositionText,
        range: 1..3,
        text: "xy".to_owned(),
        selection: 3..3,
        composition: Some(1..3),
    });
    assert_eq!(state.value(), "axyd");
    assert_eq!(state.selection(), 3..3);
    assert!(!state.reversed());
    assert_eq!(state.composition(), Some(1..3));
}

#[test]
fn unmark_keeps_the_text() {
    let mut state = caret_at("abk", 3);
    state.composition = Some(2..3);
    state.unmark();
    assert_eq!(state.composition(), None);
    assert_eq!(state.value(), "abk");
}

#[rstest]
#[case::recorded(
    &[(0, 0.0), (1, 6.0), (2, 12.0)],
    Some(Bounds::from_corners(point(px(16.), px(20.)), point(px(22.), px(36.))))
)]
#[case::unrecorded(&[], None)]
fn bounds_of_reads_the_recorded_layout(
    #[case] stops: &[(usize, f32)],
    #[case] want: Option<Bounds<Pixels>>,
) {
    assert_eq!(laid_out(10.0, stops).bounds_of(&(1..2)), want);
}

#[rstest]
#[case(InputType::InsertText, "insertText")]
#[case(InputType::InsertFromPaste, "insertFromPaste")]
#[case(InputType::InsertCompositionText, "insertCompositionText")]
#[case(InputType::InsertLineBreak, "insertLineBreak")]
#[case(InputType::DeleteContentBackward, "deleteContentBackward")]
#[case(InputType::DeleteContentForward, "deleteContentForward")]
#[case(InputType::DeleteByCut, "deleteByCut")]
fn input_type_names_follow_input_events(#[case] input_type: InputType, #[case] want: &str) {
    assert_eq!(input_type.as_str(), want);
}
