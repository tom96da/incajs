// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[gpui::test]
fn a_relayed_notification_reaches_the_app_in_the_window(cx: &mut TestAppContext) {
    let (capturing, _) = capture();
    let (window, sender) = serve_idle(cx, RECEIVES_DEV_EVENTS, TEST_ENTRY_PATH, &capturing);

    sender
        .send_blocking(line(
            &json!({ "method": "testEvent", "params": { "a": 1 } }),
        ))
        .unwrap();
    cx.run_until_parked();

    let received = eval_in_window(cx, window, "JSON.stringify(globalThis.received);");
    assert_eq!(received, r#"[["testEvent","{\"a\":1}"]]"#);
}

#[gpui::test]
fn shutdown_is_answered_and_later_lines_are_ignored(cx: &mut TestAppContext) {
    let (capturing, _) = capture();
    let (_, sender) = serve_idle(cx, "", TEST_ENTRY_PATH, &capturing);

    request(&sender, 1, "shutdown");
    request(&sender, 2, "nope");
    cx.run_until_parked();

    let sent = capturing.0.borrow();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].contains(r#""id":1"#));
    assert!(sent[0].contains(r#""result":null"#));
    assert_eq!(sender.receiver_count(), 0, "the loop ended");
}

#[gpui::test]
fn a_closed_channel_ends_the_loop(cx: &mut TestAppContext) {
    let (capturing, _) = capture();
    let (_, sender) = serve_idle(cx, "", TEST_ENTRY_PATH, &capturing);
    assert_eq!(sender.receiver_count(), 1, "the loop is waiting");

    sender.close();
    cx.run_until_parked();

    assert_eq!(sender.receiver_count(), 0, "the loop ended");
    assert!(capturing.0.borrow().is_empty());
}

#[gpui::test]
fn an_unusable_line_is_answered_and_the_loop_keeps_serving(cx: &mut TestAppContext) {
    let (capturing, _) = capture();
    let (_, sender) = serve_idle(cx, "", TEST_ENTRY_PATH, &capturing);

    for (line, code) in [("not json", "-32700"), ("[1]", "-32600")] {
        capturing.0.borrow_mut().clear();
        sender.send_blocking(line.to_owned()).unwrap();
        cx.run_until_parked();

        let sent = capturing.0.borrow();
        assert_eq!(sent.len(), 1, "{line}");
        assert!(sent[0].contains(&format!(r#""code":{code}"#)), "{line}");
        assert!(sent[0].contains(r#""id":null"#), "{line}");
        assert_eq!(sender.receiver_count(), 1, "{line}");
    }

    capturing.0.borrow_mut().clear();
    sender.send_blocking("  ".to_owned()).unwrap();
    cx.run_until_parked();
    assert!(capturing.0.borrow().is_empty());
    assert_eq!(sender.receiver_count(), 1);
}

#[gpui::test]
fn an_unknown_request_line_is_answered_method_not_found(cx: &mut TestAppContext) {
    let (capturing, _) = capture();
    let (_, sender) = serve_idle(cx, "", TEST_ENTRY_PATH, &capturing);

    request(&sender, 3, "nope");
    cx.run_until_parked();

    let sent = capturing.0.borrow();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].contains(r#""code":-32601"#));
    assert!(sent[0].contains(r#""id":3"#));
    assert!(sent[0].contains(r#""message":"unknown method: nope""#));
    assert_eq!(sender.receiver_count(), 1, "the loop keeps serving");
}

#[test]
fn a_panic_is_written_as_one_internal_error_line() {
    let mut out = Vec::new();

    write_panic_line(&mut out, "boom\nsecond line");

    let text = String::from_utf8(out).unwrap();
    assert_eq!(text.lines().count(), 1);
    let decoded: Value = serde_json::from_str(&text).unwrap();
    assert!(decoded["id"].is_null());
    assert_eq!(decoded["error"]["code"], -32603);
    assert_eq!(decoded["error"]["message"], "boom\nsecond line");
}
