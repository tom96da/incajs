// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

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
    let entry = ScratchEntry::write("pending-reload", &records_until_go("new"));
    let (capturing, _) = capture();
    let first = format!("globalThis.marker = 'old';\n{}", mounts_a_div(300, 150));
    let (window, sender) = serve_idle(cx, &first, entry.path(), &capturing);

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
    let entry = ScratchEntry::write("relay-pending", &records_until_go("new"));
    let (capturing, _) = capture();
    let (window, sender) = serve_idle(cx, RECEIVES_DEV_EVENTS, entry.path(), &capturing);

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
    let entry = ScratchEntry::write("superseded", NEVER_SETTLES);
    let (capturing, _) = capture();
    let (window, sender) = serve_idle(cx, "", entry.path(), &capturing);
    request(&sender, 1, "reload");
    cx.run_until_parked();

    std::fs::write(&entry.0, records_until_go("second")).unwrap();
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
fn a_pending_reload_that_rejects_is_answered_thrown_and_the_old_app_stays(cx: &mut TestAppContext) {
    let entry = ScratchEntry::write(
        "rejected-reload",
        r"
        await new Promise((_, reject) => {
            __inca_dev__.receive = () => reject(new Error('late'));
        });
        ",
    );
    let (capturing, _) = capture();
    let first = format!("globalThis.marker = 'old';\n{}", mounts_a_div(300, 150));
    let (window, sender) = serve_idle(cx, &first, entry.path(), &capturing);

    request(&sender, 7, "reload");
    notify(&sender, "go");
    cx.run_until_parked();

    let sent = written(&capturing);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["id"], 7);
    assert_eq!(sent[0]["error"]["code"], -32000);
    assert_eq!(sent[0]["error"]["message"], "Error: late");
    assert!(sent[0]["error"]["data"]["stack"].is_string());
    assert_eq!(eval_in_window(cx, window, "globalThis.marker;"), "old");
}

#[gpui::test]
fn a_job_error_is_reported_once_when_a_pending_reload_rejects_after_it(cx: &mut TestAppContext) {
    let entry = ScratchEntry::write(
        "side-then-reject",
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
    let (_, sender) = serve_idle(cx, "", entry.path(), &capturing);

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
    let entry = ScratchEntry::write("sync-throw", &records_until_go("second"));
    let (capturing, _) = capture();
    let (window, sender) = serve_idle(cx, "", entry.path(), &capturing);
    request(&sender, 1, "reload");
    cx.run_until_parked();

    std::fs::write(&entry.0, "throw new Error('sync');").unwrap();
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
    let entry = ScratchEntry::write("held-reload", NEVER_SETTLES);
    let (capturing, writer) = capture();
    let sender = serve(
        cx,
        "__inca_dev__.setLoadFailed(true); await new Promise(() => {});",
        entry.path(),
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
    let entry = ScratchEntry::write("keeps-deadline", NEVER_SETTLES);
    let (capturing, writer) = capture();
    let sender = serve(cx, NEVER_SETTLES, entry.path(), &writer);
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
    let entry = ScratchEntry::write("timeout-ids", &records_until_go("second"));
    let (capturing, writer) = capture();
    let sender = serve(cx, NEVER_SETTLES, entry.path(), &writer);
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
    let entry = ScratchEntry::write("shutdown-b", NEVER_SETTLES);
    let (capturing, _) = capture();
    let (_, sender) = serve_idle(cx, "", entry.path(), &capturing);
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
    let entry = ScratchEntry::write("queued-settles", &records_until_go("second"));
    let (capturing, writer) = capture();
    let sender = serve(cx, NEVER_SETTLES, entry.path(), &writer);
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

/// A directory holding just this test's entry and its own `inca.json`,
/// unlike [`ScratchEntry`] whose files all share the OS temp root as
/// their `config::read` parent. That is fine for entries with no config, but
/// not for a config a test needs isolated to itself.
struct ScratchApp(std::path::PathBuf);

impl ScratchApp {
    fn write(name: &str, source: &str, config_json: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "inca-host-dev-test-app-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("bundle.js"), source).unwrap();
        std::fs::write(dir.join("inca.json"), config_json).unwrap();
        Self(dir)
    }

    fn entry(&self) -> String {
        self.0.join("bundle.js").to_str().unwrap().to_owned()
    }
}

impl Drop for ScratchApp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[gpui::test]
fn a_reload_resets_a_fixed_size_windows_auto_resize_latch(cx: &mut TestAppContext) {
    let entry = ScratchEntry::write("resize", &mounts_a_div(400, 200));
    let (capturing, _) = capture();
    let (window, sender) = serve_idle(cx, &mounts_a_div(300, 150), entry.path(), &capturing);
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
fn a_reload_resets_the_width_latch_when_only_the_width_fired(cx: &mut TestAppContext) {
    let entry = ScratchEntry::write("resize-width", &mounts_a_div_sized("width", 400));
    let (capturing, _) = capture();
    let (window, sender) = serve_idle(
        cx,
        &mounts_a_div_sized("width", 300),
        entry.path(),
        &capturing,
    );
    assert_eq!(bounds_of(cx, window), (300.0, 600.0));

    request(&sender, 1, "reload");
    cx.run_until_parked();

    assert_eq!(bounds_of(cx, window), (400.0, 600.0));
}

#[gpui::test]
fn a_reload_resets_the_height_latch_when_only_the_height_fired(cx: &mut TestAppContext) {
    let entry = ScratchEntry::write("resize-height", &mounts_a_div_sized("height", 200));
    let (capturing, _) = capture();
    let (window, sender) = serve_idle(
        cx,
        &mounts_a_div_sized("height", 150),
        entry.path(),
        &capturing,
    );
    assert_eq!(bounds_of(cx, window), (800.0, 150.0));

    request(&sender, 1, "reload");
    cx.run_until_parked();

    assert_eq!(bounds_of(cx, window), (800.0, 200.0));
}

#[gpui::test]
fn a_reload_does_not_reset_a_resizable_windows_manual_resize(cx: &mut TestAppContext) {
    let app = ScratchApp::write(
        "resizable",
        &mounts_a_div(400, 200),
        r#"{"window":{"resizable":true}}"#,
    );
    let (capturing, _) = capture();
    let (window, sender) = serve_idle(cx, &mounts_a_div(300, 150), &app.entry(), &capturing);
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
fn a_reload_line_is_answered_null_and_swaps_the_session_in(cx: &mut TestAppContext) {
    let entry = ScratchEntry::write("serve-reload", &mounts_a_div(400, 200));
    let (capturing, _) = capture();
    let (window, sender) = serve_idle(cx, &mounts_a_div(300, 150), entry.path(), &capturing);

    request(&sender, 7, "reload");
    cx.run_until_parked();

    let sent = capturing.0.borrow();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].contains(r#""id":7"#));
    assert!(sent[0].contains(r#""result":null"#));
    assert_eq!(bounds_of(cx, window), (400.0, 200.0));
    assert_eq!(sender.receiver_count(), 1, "the loop keeps serving");
}

fn mounts_a_div_sized(property: &str, value: u32) -> String {
    format!(
        r"
            const node = __inca_native__.createNode('div');
            __inca_native__.appendChild(__inca_native__.rootNodeId(), node);
            __inca_native__.setStyle(node, '{property}', {value});
        "
    )
}
