// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use gpui::{KeyUpEvent, PlatformInput};
use inca_test_macros::gpui_cases;
use rstest::rstest;

/// Cancels `event` at `node`.
fn cancel(s: &InputScene, node: NodeId, event: &str) {
    listen(&s.dispatcher, node, event, "(e) => e.preventDefault()");
}

/// Sends a key release.
fn key_up(s: &InputScene, cx: &mut TestAppContext, key: &str) {
    let keystroke = Keystroke::parse(key).unwrap().with_simulated_ime();
    s.in_window(cx, |_, window, cx| {
        window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke }), cx);
    });
    cx.run_until_parked();
}

#[gpui::test]
fn a_typed_key_fires_keydown_keypress_beforeinput_input_in_order(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    record(
        &s,
        s.input,
        &["keydown", "keypress", "beforeinput", "input", "keyup"],
    );
    s.focus(cx, s.input);
    s.key(cx, "x");
    key_up(&s, cx, "x");
    assert_eq!(
        types(&s),
        ["keydown", "keypress", "beforeinput", "input", "keyup"]
    );
}

#[gpui::test]
fn a_keypress_of_enter_carries_the_character_code(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    listen(
        &s.dispatcher,
        s.input,
        "keypress",
        "(e) => out.push([e.key, e.charCode, e.keyCode, e.which, e.cancelable])",
    );
    s.focus(cx, s.input);
    s.key(cx, "enter");
    assert_eq!(
        s.engine.eval::<String>("JSON.stringify(out)").unwrap(),
        r#"[["Enter",13,13,13,true]]"#
    );
}

#[gpui_cases]
#[gpui::test]
#[case::tab("tab")]
#[case::escape("escape")]
#[case::backspace("backspace")]
fn a_key_that_types_no_character_fires_no_keypress(cx: &mut TestAppContext, pressed: &str) {
    let s = input_scene(cx, "ab");
    record(&s, s.input, &["keydown", "keypress"]);
    s.focus(cx, s.input);
    s.key(cx, pressed);
    assert_eq!(types(&s), ["keydown"], "{pressed}");
}

#[gpui_cases]
#[gpui::test]
#[case::control("ctrl-enter")]
#[case::alt("alt-enter")]
#[case::meta("cmd-enter")]
fn enter_held_with_control_alt_or_meta_fires_no_keypress(cx: &mut TestAppContext, pressed: &str) {
    let s = input_scene(cx, "ab");
    record(&s, s.input, &["keydown", "keypress"]);
    s.focus(cx, s.input);
    s.key(cx, pressed);
    assert_eq!(types(&s), ["keydown"], "{pressed}");
}

#[rstest]
#[case::option_s_on_macos("alt-s->ß", true, true)]
#[case::alt_a_off_macos("alt-a->a", false, false)]
#[case::control_option_s_on_macos("ctrl-alt-s->ß", true, false)]
#[case::command_s_on_macos("cmd-s->s", true, false)]
#[case::alt_enter_on_macos("alt-enter", true, false)]
#[case::enter("enter", false, true)]
#[case::letter("x->x", false, true)]
fn types_character_follows_the_modifiers_and_the_platform(
    #[case] pressed: &str,
    #[case] macos: bool,
    #[case] want: bool,
) {
    let keystroke = Keystroke::parse(pressed).unwrap();

    assert_eq!(
        crate::dispatch::sink::types_character(&keystroke, macos),
        want,
        "{pressed}"
    );
}

#[test]
fn an_empty_key_char_types_no_character() {
    let keystroke = Keystroke {
        modifiers: gpui::Modifiers::none(),
        key: "x".to_owned(),
        key_char: Some(String::new()),
    };

    assert!(!crate::dispatch::sink::types_character(&keystroke, true));
}

#[rstest]
#[case::delete("\u{7f}")]
#[case::c1_control("\u{85}")]
fn a_control_character_as_key_char_types_no_character(#[case] key_char: &str) {
    let keystroke = Keystroke {
        modifiers: gpui::Modifiers::none(),
        key: "x".to_owned(),
        key_char: Some(key_char.to_owned()),
    };

    assert!(!crate::dispatch::sink::types_character(&keystroke, true));
}

#[cfg(target_os = "macos")]
#[gpui_cases]
#[gpui::test]
#[case::option_s("alt-s->ß", "ß", 223)]
fn an_option_key_fires_keypress_with_the_composed_character(
    cx: &mut TestAppContext,
    pressed: &str,
    key: &str,
    code: u32,
) {
    let s = input_scene(cx, "");
    listen(
        &s.dispatcher,
        s.input,
        "keypress",
        "(e) => out.push([e.key, e.charCode, e.keyCode, e.which])",
    );
    s.focus(cx, s.input);
    s.key(cx, pressed);
    assert_eq!(
        s.engine.eval::<String>("JSON.stringify(out)").unwrap(),
        format!(r#"[["{key}",{code},{code},{code}]]"#),
        "{pressed}"
    );
}

#[cfg(not(target_os = "macos"))]
#[gpui_cases]
#[gpui::test]
#[case::alt_a("alt-a->a")]
fn an_alt_key_fires_no_keypress_off_macos(cx: &mut TestAppContext, pressed: &str) {
    let s = input_scene(cx, "");
    record(&s, s.input, &["keydown", "keypress"]);
    s.focus(cx, s.input);
    s.key(cx, pressed);
    assert_eq!(types(&s), ["keydown"], "{pressed}");
}

#[gpui::test]
fn a_canceled_keydown_fires_no_keypress(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    record(&s, s.input, &["keydown", "keypress"]);
    cancel(&s, s.input, "keydown");
    s.focus(cx, s.input);
    s.key(cx, "x");
    assert_eq!(types(&s), ["keydown"]);
}

#[gpui::test]
fn a_canceled_keypress_types_nothing(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    record(&s, s.input, &["keydown", "keypress", "beforeinput"]);
    cancel(&s, s.input, "keypress");
    s.focus(cx, s.input);
    s.key(cx, "x");
    assert_eq!(types(&s), ["keydown", "keypress"]);
    assert_eq!(s.state().value(), "");
}

#[gpui::test]
fn enter_in_an_input_fires_keypress_then_beforeinput(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    record(&s, s.input, &["keydown", "keypress", "beforeinput"]);
    s.focus(cx, s.input);
    s.key(cx, "enter");
    assert_eq!(types(&s), ["keydown", "keypress", "beforeinput"]);
}

#[gpui::test]
fn a_canceled_keypress_of_enter_in_an_input_skips_beforeinput(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    record(&s, s.input, &["keydown", "keypress", "beforeinput"]);
    cancel(&s, s.input, "keypress");
    s.focus(cx, s.input);
    s.key(cx, "enter");
    assert_eq!(types(&s), ["keydown", "keypress"]);
}

#[gpui::test]
fn a_canceled_keypress_of_space_on_a_button_leaves_the_click_on_key_up(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    record(&s, s.button, &["keypress", "click"]);
    cancel(&s, s.button, "keypress");
    s.focus(cx, s.button);
    s.key(cx, "space");
    key_up(&s, cx, "space");
    assert_eq!(types(&s), ["keypress", "click"]);
}

#[gpui::test]
fn no_keypress_fires_during_a_composition(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    record(&s, s.input, &["keydown", "keypress"]);
    s.focus(cx, s.input);
    s.request(
        cx,
        InputRequest::Mark {
            range: None,
            text: "k".to_owned(),
            selected: None,
        },
    );
    s.key(cx, "x");
    assert_eq!(types(&s), ["keydown"]);
}

/// The focus a `keydown` asks for applies at the next frame, so `keypress` goes to the node that
/// had the focus.
#[gpui::test]
fn a_keydown_that_asks_for_another_focus_sends_keypress_to_the_old_target(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    record(&s, s.input, &["keypress"]);
    listen(
        &s.dispatcher,
        s.button,
        "keypress",
        "(e) => out.push('button')",
    );
    listen(
        &s.dispatcher,
        s.input,
        "keydown",
        &format!("(e) => __inca_native__.focusNode({})", s.button),
    );
    s.focus(cx, s.input);
    s.key(cx, "x");
    assert_eq!(types(&s), ["keypress"]);
    assert_eq!(
        s.engine.eval::<String>("JSON.stringify(out)").unwrap(),
        "[]"
    );
}

/// The `destroyNode` of the listener releases the listeners of the target, so the later `keypress`
/// walk finds none and fires nothing. The test pins that the key path survives a target that is
/// gone.
#[gpui::test]
fn a_keydown_that_destroys_its_node_fires_no_keypress(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    record(&s, s.input, &["keypress"]);
    listen(
        &s.dispatcher,
        s.input,
        "keydown",
        &format!("(e) => __inca_native__.destroyNode({})", s.input),
    );
    s.focus(cx, s.input);
    s.key(cx, "x");
    assert!(types(&s).is_empty());
}
