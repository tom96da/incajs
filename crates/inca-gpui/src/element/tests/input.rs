// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::float_cmp, clippy::unwrap_used)]

use super::*;
use crate::element::input::text_runs;
use std::ops::Range;

use gpui::{InputHandler, Keystroke, TextStyle, point, size};
use inca_test_macros::gpui_cases;
use rstest::rstest;

/// An `input` root 100 px wide at text size 10: 6 px per char, 16 px tall.
fn input_tree(width: f64) -> VirtualTree {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("input").unwrap();
    tree.set_style(id, "width", width).unwrap();
    tree.set_style(id, "text_size", 10.0).unwrap();
    tree
}

/// A probe holding `value` as the text state of node 0.
fn probe_with(tree: VirtualTree, value: &str) -> Probe {
    let probe = probe_of(tree);
    let mut state = InputState::default();
    state.set_value(value);
    probe.inputs.borrow_mut().insert(0, state);
    probe
}

fn drawn(cx: &mut VisualTestContext, probe: &Probe) {
    cx.draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
        render_tree_with_events(&probe.tree.borrow(), 0, probe).unwrap()
    });
}

fn state_of(probe: &Probe) -> InputState {
    probe.inputs.borrow()[&0].clone()
}

struct PlainView(VirtualTree);

impl Render for PlainView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        render_tree(&self.0, 0).unwrap()
    }
}

#[gpui::test]
fn an_input_without_a_sink_registers_no_input_handler(cx: &mut TestAppContext) {
    let mut tree = input_tree(100.0);
    tree.set_attribute(0, "placeholder", "hint").unwrap();
    let window = cx.add_window(|_, _| PlainView(tree));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let handled =
        cx.update(|window, cx| window.dispatch_keystroke(Keystroke::parse("x").unwrap(), cx));
    assert!(!handled);
}

#[gpui::test]
fn an_input_records_a_caret_stop_per_char(cx: &mut TestAppContext) {
    let probe = probe_with(input_tree(100.0), "ab");
    drawn(cx.add_empty_window(), &probe);
    let state = probe.inputs.borrow()[&0].clone();
    let layout = state.layout();
    assert_eq!(layout.stops, [(0, 0.), (1, 6.), (2, 12.)]);
    assert_eq!(layout.x, 0.);
    assert_eq!(layout.height, 16.);
}

#[gpui::test]
fn an_empty_input_records_one_stop(cx: &mut TestAppContext) {
    let mut tree = input_tree(100.0);
    tree.set_attribute(0, "placeholder", "hint").unwrap();
    let probe = probe_of(tree);
    drawn(cx.add_empty_window(), &probe);
    assert_eq!(state_of(&probe).layout().stops, [(0, 0.)]);
}

#[gpui::test]
fn an_input_shifts_long_text_to_show_the_caret(cx: &mut TestAppContext) {
    let probe = probe_with(input_tree(30.0), "abcdefghij");
    drawn(cx.add_empty_window(), &probe);
    let state = state_of(&probe);
    assert_eq!(state.layout().scroll, 31.);
    assert_eq!(state.layout().x, -31.);
}

#[gpui::test]
fn a_focused_input_takes_typed_text(cx: &mut TestAppContext) {
    let probe = probe_with(input_tree(100.0), "");
    let view = probe.clone();
    let window = cx.add_window(|_, _| ProbeView(view));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let handle = cx.update(|_, cx| cx.focus_handle());
    *probe.focus.borrow_mut() = Some((0, handle.clone()));
    cx.update(|window, cx| {
        handle.focus(window, cx);
        window.draw(cx).clear(cx);
    });
    cx.simulate_input("x");
    let want = InputRequest::Replace {
        range: None,
        text: "x".to_owned(),
    };
    assert_eq!(*probe.requests.borrow(), [(0, want)]);
}

/// The handler of node 0 over a probe whose state is `state`.
fn handler_over(state: Option<InputState>, editable: bool) -> (Probe, TextInputHandler<Probe>) {
    let probe = probe();
    if let Some(state) = state {
        probe.inputs.borrow_mut().insert(0, state);
    }
    let handler = TextInputHandler::new(probe.clone(), 0, editable);
    (probe, handler)
}

fn state_with(value: &str, selection: Range<usize>, reversed: bool) -> InputState {
    let mut state = InputState::default();
    state.set_value(value);
    if reversed {
        state.select(selection.end, selection.start);
    } else {
        state.select(selection.start, selection.end);
    }
    state
}

#[gpui::test]
fn the_handler_reports_the_selection_in_utf16(cx: &mut TestAppContext) {
    let (_, mut handler) = handler_over(Some(state_with("a😀b", 1..5, true)), true);
    let got = cx
        .add_empty_window()
        .update(|window, cx| handler.selected_text_range(false, window, cx))
        .unwrap();
    assert_eq!(got.range, 1..3);
    assert!(got.reversed);
}

#[gpui_cases]
#[gpui::test]
#[case::composing(&["かな"], Some(0..2))]
#[case::idle(&[], None)]
fn the_handler_reports_the_composition_in_utf16(
    cx: &mut TestAppContext,
    #[case] marks: &[&str],
    #[case] want: Option<Range<usize>>,
) {
    let mut state = InputState::default();
    for text in marks {
        state.apply(&state.compose(None, text, None));
    }
    let (_, mut handler) = handler_over(Some(state), true);
    let got = cx
        .add_empty_window()
        .update(|window, cx| handler.marked_text_range(window, cx));
    assert_eq!(got, want);
}

#[gpui::test]
fn the_handler_returns_the_text_of_an_adjusted_range(cx: &mut TestAppContext) {
    let (_, mut handler) = handler_over(Some(state_with("a😀", 0..0, false)), true);
    let mut adjusted = None;
    let got = cx
        .add_empty_window()
        .update(|window, cx| handler.text_for_range(1..2, &mut adjusted, window, cx));
    assert_eq!(got.as_deref(), Some("😀"));
    assert_eq!(adjusted, Some(1..3));
}

#[gpui_cases]
#[gpui::test]
#[case::replace(
    |h, w, cx| h.replace_text_in_range(Some(1..2), "x", w, cx),
    InputRequest::Replace { range: Some(1..2), text: "x".to_owned() }
)]
#[case::mark(
    |h, w, cx| h.replace_and_mark_text_in_range(None, "k", Some(0..1), w, cx),
    InputRequest::Mark { range: None, text: "k".to_owned(), selected: Some(0..1) }
)]
#[case::unmark(|h, w, cx| h.unmark_text(w, cx), InputRequest::Unmark)]
#[case::select(|h, w, cx| h.set_selected_text_range(0..1, w, cx), InputRequest::Select(0..1))]
fn the_handler_forwards_each_write(
    cx: &mut TestAppContext,
    #[case] call: fn(&mut TextInputHandler<Probe>, &mut Window, &mut App),
    #[case] request: InputRequest,
) {
    let (probe, mut handler) = handler_over(None, true);
    cx.add_empty_window()
        .update(|window, cx| call(&mut handler, window, cx));
    assert_eq!(*probe.requests.borrow(), [(0, request)]);
}

/// A state that recorded a line at window x 10, y 20, 6 px per char.
fn recorded(value: &str) -> InputState {
    let mut state = state_with(value, 0..0, false);
    let stops = crate::input::stops_of(value, |at| f32::from(u16::try_from(at).unwrap()) * 6.0);
    state.record(InputLayout {
        x: 10.0,
        y: 20.0,
        height: 16.0,
        scroll: 0.0,
        stops,
    });
    state
}

#[gpui::test]
fn the_handler_reports_the_box_of_a_range(cx: &mut TestAppContext) {
    let (_, mut handler) = handler_over(Some(recorded("abc")), true);
    let got = cx
        .add_empty_window()
        .update(|window, cx| handler.bounds_for_range(1..2, window, cx));
    let want = gpui::Bounds::from_corners(point(px(16.), px(20.)), point(px(22.), px(36.)));
    assert_eq!(got, Some(want));
}

#[gpui::test]
fn the_handler_maps_a_point_to_a_utf16_offset(cx: &mut TestAppContext) {
    let (_, mut handler) = handler_over(Some(recorded("a😀")), true);
    let got = cx.add_empty_window().update(|window, cx| {
        handler.character_index_for_point(point(px(19.), px(99.)), window, cx)
    });
    assert_eq!(got, Some(1));
}

#[gpui::test]
fn the_handler_reports_the_length_in_utf16(cx: &mut TestAppContext) {
    let (_, mut handler) = handler_over(Some(state_with("a😀", 0..0, false)), true);
    let got = cx
        .add_empty_window()
        .update(|window, cx| handler.text_length_utf16(window, cx));
    assert_eq!(got, Some(3));
}

#[gpui_cases]
#[gpui::test]
#[case::editable(true)]
#[case::readonly(false)]
fn the_handler_accepts_text_only_when_editable(cx: &mut TestAppContext, #[case] editable: bool) {
    let (_, mut handler) = handler_over(None, editable);
    let got = cx
        .add_empty_window()
        .update(|window, cx| handler.accepts_text_input(window, cx));
    assert_eq!(got, editable);
}

#[gpui_cases]
#[gpui::test]
#[case::editable(true)]
#[case::readonly(false)]
fn the_handler_prefers_ime_only_when_editable(cx: &mut TestAppContext, #[case] editable: bool) {
    let (_, mut handler) = handler_over(None, editable);
    let got = cx
        .add_empty_window()
        .update(|window, cx| handler.prefers_ime_for_printable_keys(window, cx));
    assert_eq!(got, editable);
}

#[gpui::test]
fn the_handler_of_an_unknown_node_reports_nothing(cx: &mut TestAppContext) {
    let (_, mut handler) = handler_over(None, true);
    let got = cx
        .add_empty_window()
        .update(|window, cx| handler.selected_text_range(false, window, cx));
    assert!(got.is_none());
}

#[rstest]
#[case::none(None, &[(5, false)])]
#[case::middle(Some(1..3), &[(1, false), (2, true), (2, false)])]
#[case::at_end(Some(3..5), &[(3, false), (2, true)])]
fn text_runs_underline_the_composition(
    #[case] composition: Option<Range<usize>>,
    #[case] want: &[(usize, bool)],
) {
    let runs = text_runs(5, composition, &TextStyle::default().to_run(5));
    let got: Vec<_> = runs
        .iter()
        .map(|run| (run.len, run.underline.is_some()))
        .collect();
    assert_eq!(got, want);
}
