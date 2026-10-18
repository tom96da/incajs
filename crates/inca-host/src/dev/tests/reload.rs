// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

/// A pending entry that settles on `go`, records every notification in
/// `globalThis.got`, and says which session it is in `globalThis.marker`.
fn records_until_go(marker: &str) -> String {
    format!(
        r"
        globalThis.marker = '{marker}'; globalThis.got = [];
        await new Promise((r) => {{ __inca_dev__.receive = (m) => {{
            globalThis.got.push(m); if (m === 'go') r();
            }};
        }});
        {}
        ",
        mounts_a_div(400, 200)
    )
}

#[gpui::test]
fn a_pending_reload_is_answered_when_it_settles_and_the_old_app_stays_until_then(
    cx: &mut TestAppContext,
) {
    let dir = ScratchDir::new("pending-reload");
    let entry = write_entry(&dir, &records_until_go("new"));
    let (capturing, _) = capture();
    let first = format!("globalThis.marker = 'old';\n{}", mounts_a_div(300, 150));
    let (window, sender) = serve_idle(cx, &first, &entry, &capturing);

    request(&sender, 5, "reload");
    cx.run_until_parked();
    assert!(capturing.0.borrow().is_empty(), "nothing answers yet");
    assert_eq!(eval_in_window(cx, window, "globalThis.marker;"), "old");
    assert_eq!(bounds_of(cx, window), (300.0, 150.0));

    notify(&sender, "go");
    cx.run_until_parked();

    assert_eq!(
        written(&capturing),
        [json!({ "jsonrpc": "2.0", "id": 5, "result": null })]
    );
    assert_eq!(eval_in_window(cx, window, "globalThis.marker;"), "new");
    assert_eq!(bounds_of(cx, window), (400.0, 200.0));
}

#[gpui::test]
fn a_notification_during_a_pending_reload_reaches_the_new_app_only(cx: &mut TestAppContext) {
    let dir = ScratchDir::new("relay-pending");
    let entry = write_entry(&dir, &records_until_go("new"));
    let (capturing, _) = capture();
    let (window, sender) = serve_idle(cx, RECEIVES_DEV_EVENTS, &entry, &capturing);

    request(&sender, 1, "reload");
    notify(&sender, "first");
    cx.run_until_parked();
    assert_eq!(
        eval_in_window(cx, window, "String(globalThis.received.length);"),
        "0",
        "the old app saw nothing"
    );

    notify(&sender, "go");
    cx.run_until_parked();
    assert_eq!(
        eval_in_window(cx, window, "JSON.stringify(globalThis.got);"),
        r#"["first","go"]"#
    );
}

#[gpui::test]
fn a_later_reload_replaces_a_pending_one_and_both_ids_get_its_outcome(cx: &mut TestAppContext) {
    let dir = ScratchDir::new("superseded");
    let entry = write_entry(&dir, NEVER_SETTLES);
    let (capturing, _) = capture();
    let (window, sender) = serve_idle(cx, "", &entry, &capturing);
    request(&sender, 1, "reload");
    cx.run_until_parked();

    std::fs::write(&entry, records_until_go("second")).unwrap();
    request(&sender, 2, "reload");
    notify(&sender, "go");
    cx.run_until_parked();

    assert_eq!(
        written(&capturing),
        [
            json!({ "jsonrpc": "2.0", "id": 1, "result": null }),
            json!({ "jsonrpc": "2.0", "id": 2, "result": null }),
        ]
    );
    assert_eq!(eval_in_window(cx, window, "globalThis.marker;"), "second");
}

#[gpui::test]
fn a_rejected_pending_reload_is_answered_with_its_error(cx: &mut TestAppContext) {
    let dir = ScratchDir::new("rejected-reload");
    let entry = write_entry(
        &dir,
        r"
        await new Promise((_, reject) => {
            __inca_dev__.receive = () => reject(new Error('late'));
        });
        ",
    );
    let (capturing, _) = capture();
    let first = format!("globalThis.marker = 'old';\n{}", mounts_a_div(300, 150));
    let (_, sender) = serve_idle(cx, &first, &entry, &capturing);

    request(&sender, 7, "reload");
    notify(&sender, "go");
    cx.run_until_parked();

    let sent = written(&capturing);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["id"], 7);
    assert_eq!(sent[0]["error"]["code"], -32000);
    assert_eq!(sent[0]["error"]["message"], "Error: late");
    assert!(sent[0]["error"]["data"]["stack"].is_string());
}

#[gpui::test]
fn a_rejected_pending_reload_keeps_the_old_app(cx: &mut TestAppContext) {
    let dir = ScratchDir::new("rejected-reload");
    let entry = write_entry(
        &dir,
        r"
        await new Promise((_, reject) => {
            __inca_dev__.receive = () => reject(new Error('late'));
        });
        ",
    );
    let (capturing, _) = capture();
    let first = format!("globalThis.marker = 'old';\n{}", mounts_a_div(300, 150));
    let (window, sender) = serve_idle(cx, &first, &entry, &capturing);

    request(&sender, 7, "reload");
    notify(&sender, "go");
    cx.run_until_parked();

    assert_eq!(eval_in_window(cx, window, "globalThis.marker;"), "old");
}

#[gpui::test]
fn a_job_error_is_reported_once_when_a_pending_reload_rejects_after_it(cx: &mut TestAppContext) {
    let dir = ScratchDir::new("side-then-reject");
    let entry = write_entry(
        &dir,
        r"
        await new Promise((_, reject) => {
            __inca_dev__.receive = () => {
                queueMicrotask(() => { throw new Error('side'); });
                Promise.resolve().then(() => reject(new Error('late')));
            };
        });
        ",
    );
    let (capturing, _) = capture();
    let (_, sender) = serve_idle(cx, "", &entry, &capturing);

    request(&sender, 7, "reload");
    notify(&sender, "go");
    cx.run_until_parked();

    let sent = written(&capturing);
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0]["method"], "appError");
    assert_eq!(sent[0]["params"]["message"], "Error: side");
    assert_eq!(sent[1]["id"], 7);
    assert_eq!(sent[1]["error"]["message"], "Error: late");
}

#[gpui::test]
fn a_reload_that_throws_at_once_is_answered_while_an_earlier_one_stays_pending(
    cx: &mut TestAppContext,
) {
    let dir = ScratchDir::new("sync-throw");
    let entry = write_entry(&dir, &records_until_go("second"));
    let (capturing, _) = capture();
    let (window, sender) = serve_idle(cx, "", &entry, &capturing);
    request(&sender, 1, "reload");
    cx.run_until_parked();

    std::fs::write(&entry, "throw new Error('sync');").unwrap();
    request(&sender, 2, "reload");
    cx.run_until_parked();
    let sent = written(&capturing);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["id"], 2);
    assert_eq!(sent[0]["error"]["message"], "Error: sync");

    notify(&sender, "go");
    cx.run_until_parked();
    let sent = written(&capturing);
    assert_eq!(sent.len(), 2);
    assert_eq!(
        sent[1],
        json!({ "jsonrpc": "2.0", "id": 1, "result": null })
    );
    assert_eq!(eval_in_window(cx, window, "globalThis.marker;"), "second");
}

#[gpui::test]
fn a_reload_of_a_missing_file_is_answered_at_once(cx: &mut TestAppContext) {
    let (capturing, _) = capture();
    let (_, sender) = serve_idle(cx, "", "/test/missing/entry.js", &capturing);

    request(&sender, 4, "reload");
    cx.run_until_parked();

    let sent = written(&capturing);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["id"], 4);
    assert_eq!(sent[0]["error"]["code"], -32000);
    assert!(
        sent[0]["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with("failed to read /test/missing/entry.js")
    );
}

#[gpui::test]
fn a_reload_replaces_a_held_first_load_with_a_fresh_timeout(cx: &mut TestAppContext) {
    let dir = ScratchDir::new("held-reload");
    let entry = write_entry(&dir, NEVER_SETTLES);
    let (capturing, writer) = capture();
    let sender = serve(
        cx,
        "__inca_dev__.setLoadFailed(true); await new Promise(() => {});",
        &entry,
        &writer,
    );
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(5));
    cx.run_until_parked();
    assert!(the_window(cx).is_none());

    request(&sender, 9, "reload");
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(1900));
    cx.run_until_parked();
    assert!(the_window(cx).is_none());

    cx.executor().advance_clock(Duration::from_millis(200));
    cx.run_until_parked();
    assert!(the_window(cx).is_some());
    assert_eq!(
        written(&capturing),
        [
            json!({ "jsonrpc": "2.0", "method": "ready", "params": { "protocol": 0 } }),
            json!({ "jsonrpc": "2.0", "id": 9, "result": null }),
        ]
    );
}

#[gpui::test]
fn a_reload_that_replaces_a_load_that_is_not_held_keeps_the_deadline(cx: &mut TestAppContext) {
    let dir = ScratchDir::new("keeps-deadline");
    let entry = write_entry(&dir, NEVER_SETTLES);
    let (capturing, writer) = capture();
    let sender = serve(cx, NEVER_SETTLES, &entry, &writer);
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(1500));
    request(&sender, 1, "reload");
    cx.run_until_parked();
    assert!(the_window(cx).is_none());

    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();

    assert!(the_window(cx).is_some());
    assert_eq!(
        written(&capturing),
        [
            json!({ "jsonrpc": "2.0", "method": "ready", "params": { "protocol": 0 } }),
            json!({ "jsonrpc": "2.0", "id": 1, "result": null }),
        ]
    );
}

#[gpui::test]
fn the_timeout_answers_a_queued_reload_once(cx: &mut TestAppContext) {
    let dir = ScratchDir::new("timeout-ids");
    let entry = write_entry(&dir, &records_until_go("second"));
    let (capturing, writer) = capture();
    let sender = serve(cx, NEVER_SETTLES, &entry, &writer);
    request(&sender, 3, "reload");
    cx.run_until_parked();

    cx.executor().advance_clock(LOAD_TIMEOUT);
    cx.run_until_parked();
    notify(&sender, "go");
    cx.run_until_parked();

    assert_eq!(
        written(&capturing),
        [
            json!({ "jsonrpc": "2.0", "method": "ready", "params": { "protocol": 0 } }),
            json!({ "jsonrpc": "2.0", "id": 3, "result": null }),
        ]
    );
}

#[gpui::test]
fn shutdown_during_a_pending_reload_answers_only_the_shutdown(cx: &mut TestAppContext) {
    let dir = ScratchDir::new("shutdown-b");
    let entry = write_entry(&dir, NEVER_SETTLES);
    let (capturing, _) = capture();
    let (_, sender) = serve_idle(cx, "", &entry, &capturing);
    request(&sender, 1, "reload");
    request(&sender, 2, "shutdown");
    cx.run_until_parked();

    assert_eq!(
        written(&capturing),
        [json!({ "jsonrpc": "2.0", "id": 2, "result": null })]
    );
    assert_eq!(sender.receiver_count(), 0, "the loop ended");
}

#[gpui::test]
fn a_queued_reload_is_answered_when_the_first_load_settles_in_time(cx: &mut TestAppContext) {
    let dir = ScratchDir::new("queued-settles");
    let entry = write_entry(&dir, &records_until_go("second"));
    let (capturing, writer) = capture();
    let sender = serve(cx, NEVER_SETTLES, &entry, &writer);
    request(&sender, 4, "reload");
    notify(&sender, "go");
    cx.run_until_parked();

    assert_eq!(
        written(&capturing),
        [
            ready_message(),
            json!({ "jsonrpc": "2.0", "id": 4, "result": null }),
        ]
    );
}

#[gpui::test]
fn a_reload_resets_a_fixed_size_windows_auto_resize_latch(cx: &mut TestAppContext) {
    let dir = ScratchDir::new("resize");
    let entry = write_entry(&dir, &mounts_a_div(400, 200));
    let (capturing, _) = capture();
    let (window, sender) = serve_idle(cx, &mounts_a_div(300, 150), &entry, &capturing);
    assert_eq!(bounds_of(cx, window), (300.0, 150.0));

    // A relay latches auto_resized (same size, so no visible change),
    // but this is the state a reload has to reset.
    notify(&sender, "testEvent");
    request(&sender, 1, "reload");
    cx.run_until_parked();

    assert_eq!(
        bounds_of(cx, window),
        (400.0, 200.0),
        "reload must reset the latch, or the new content's size never applies"
    );
}

#[gpui::test]
fn a_reload_does_not_reset_a_resizable_windows_manual_resize(cx: &mut TestAppContext) {
    let dir = ScratchDir::new("app-resizable");
    let entry = write_entry(&dir, &mounts_a_div(400, 200));
    std::fs::write(dir.join("inca.json"), r#"{"window":{"resizable":true}}"#).unwrap();
    let (capturing, _) = capture();
    let (window, sender) = serve_idle(cx, &mounts_a_div(300, 150), &entry, &capturing);
    assert_eq!(bounds_of(cx, window), (300.0, 150.0));

    // The user grabs an edge and resizes it by hand.
    cx.update(|cx| {
        window
            .update(cx, |_, window, _| {
                window.resize(gpui::size(gpui::px(500.0), gpui::px(500.0)));
            })
            .unwrap();
    });
    assert_eq!(bounds_of(cx, window), (500.0, 500.0));

    request(&sender, 1, "reload");
    cx.run_until_parked();

    assert_eq!(
        bounds_of(cx, window),
        (500.0, 500.0),
        "a resizable window the user resized by hand must not snap back on reload"
    );
}

#[gpui::test]
fn a_reload_line_is_answered_null(cx: &mut TestAppContext) {
    let dir = ScratchDir::new("serve-reload");
    let entry = write_entry(&dir, &mounts_a_div(400, 200));
    let (capturing, _) = capture();
    let (_, sender) = serve_idle(cx, &mounts_a_div(300, 150), &entry, &capturing);

    request(&sender, 7, "reload");
    cx.run_until_parked();

    let sent = capturing.0.borrow();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].contains(r#""id":7"#));
    assert!(sent[0].contains(r#""result":null"#));
    assert_eq!(sender.receiver_count(), 1, "the loop keeps serving");
}

#[gpui::test]
fn a_reload_line_swaps_the_session_in(cx: &mut TestAppContext) {
    let dir = ScratchDir::new("serve-reload");
    let entry = write_entry(&dir, &mounts_a_div(400, 200));
    let (capturing, _) = capture();
    let (window, sender) = serve_idle(cx, &mounts_a_div(300, 150), &entry, &capturing);

    request(&sender, 7, "reload");
    cx.run_until_parked();

    assert_eq!(bounds_of(cx, window), (400.0, 200.0));
}
