// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

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

const THROWS_AFTER_QUEUEING_A_JOB: &str = r"
    globalThis.drained = false;
    __inca_dev__.receive = () => {
        Promise.resolve().then(() => {
            globalThis.drained = true;
        });
        throw new Error('rx');
    };
";

#[gpui::test]
fn a_receive_that_throws_in_the_window_is_reported(cx: &mut TestAppContext) {
    let (capturing, _) = capture();
    let (_, sender) = serve_idle(cx, THROWS_AFTER_QUEUEING_A_JOB, TEST_ENTRY_PATH, &capturing);

    notify(&sender, "probe");
    cx.run_until_parked();

    let sent = written(&capturing);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["method"], "appError");
    assert_eq!(sent[0]["params"]["message"], "Error: rx");
}

#[gpui::test]
fn a_receive_that_throws_in_the_window_still_drains_the_job_queue(cx: &mut TestAppContext) {
    let (capturing, _) = capture();
    let (window, sender) = serve_idle(cx, THROWS_AFTER_QUEUEING_A_JOB, TEST_ENTRY_PATH, &capturing);

    notify(&sender, "probe");
    cx.run_until_parked();

    assert_eq!(
        eval_in_window(cx, window, "String(globalThis.drained);"),
        "true"
    );
}

#[gpui::test]
fn shutdown_is_answered_with_null(cx: &mut TestAppContext) {
    let (capturing, _) = capture();
    let (_, sender) = serve_idle(cx, "", TEST_ENTRY_PATH, &capturing);

    request(&sender, 1, "shutdown");
    cx.run_until_parked();

    let sent = capturing.0.borrow();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].contains(r#""id":1"#));
    assert!(sent[0].contains(r#""result":null"#));
}

#[gpui::test]
fn lines_after_shutdown_are_ignored(cx: &mut TestAppContext) {
    let (capturing, _) = capture();
    let (_, sender) = serve_idle(cx, "", TEST_ENTRY_PATH, &capturing);

    request(&sender, 1, "shutdown");
    request(&sender, 2, "nope");
    cx.run_until_parked();

    assert_eq!(capturing.0.borrow().len(), 1);
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
fn an_unusable_line_gets_an_error_reply(cx: &mut TestAppContext) {
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
    }
}

#[gpui::test]
fn an_unusable_line_keeps_the_loop_serving(cx: &mut TestAppContext) {
    let (capturing, _) = capture();
    let (_, sender) = serve_idle(cx, "", TEST_ENTRY_PATH, &capturing);

    for line in ["not json", "[1]"] {
        sender.send_blocking(line.to_owned()).unwrap();
        cx.run_until_parked();

        assert_eq!(sender.receiver_count(), 1, "{line}");
    }
}

#[gpui::test]
fn a_blank_line_gets_no_reply(cx: &mut TestAppContext) {
    let (capturing, _) = capture();
    let (_, sender) = serve_idle(cx, "", TEST_ENTRY_PATH, &capturing);

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
