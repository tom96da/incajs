// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

/// An entry that settles when any notification arrives, then mounts a
/// `width` by `height` div.
fn waits_then_mounts(width: u32, height: u32) -> String {
    format!(
        "await new Promise((r) => {{ __inca_dev__.receive = () => r(); }});\n{}",
        mounts_a_div(width, height)
    )
}

#[gpui::test]
fn an_entry_that_settles_at_once_opens_the_window_before_serve_returns(cx: &mut TestAppContext) {
    let (capturing, writer) = capture();

    let _sender = serve(cx, &mounts_a_div(300, 150), TEST_ENTRY_PATH, &writer);

    assert_eq!(methods(&capturing), ["ready"]);
    let window = the_window(cx).unwrap();
    assert_eq!(bounds_of(cx, window), (300.0, 150.0));
}

#[gpui::test]
fn a_pending_entry_keeps_the_window_closed_until_a_notification_settles_it(
    cx: &mut TestAppContext,
) {
    let (capturing, writer) = capture();
    let sender = serve(cx, &waits_then_mounts(300, 150), TEST_ENTRY_PATH, &writer);
    cx.run_until_parked();
    assert!(the_window(cx).is_none());
    assert!(capturing.0.borrow().is_empty());

    notify(&sender, "go");
    cx.run_until_parked();

    assert_eq!(methods(&capturing), ["ready"]);
    let window = the_window(cx).unwrap();
    assert_eq!(bounds_of(cx, window), (300.0, 150.0));
}

#[gpui::test]
fn a_pending_entry_with_nothing_mounted_opens_the_default_window_after_the_timeout(
    cx: &mut TestAppContext,
) {
    let (capturing, writer) = capture();
    let source = format!(
        "await new Promise((r) => {{ __inca_dev__.receive = () => {{ {} r(); }}; }});",
        mounts_a_div(300, 150)
    );
    let sender = serve(cx, &source, TEST_ENTRY_PATH, &writer);
    cx.run_until_parked();

    cx.executor().advance_clock(LOAD_TIMEOUT);
    cx.run_until_parked();

    assert_eq!(methods(&capturing), ["ready"]);
    let window = the_window(cx).unwrap();
    assert_eq!(bounds_of(cx, window), (800.0, 600.0));

    notify(&sender, "mount");
    cx.run_until_parked();
    assert_eq!(bounds_of(cx, window), (300.0, 150.0));
}

#[gpui::test]
fn a_line_that_settles_nothing_does_not_extend_the_timeout(cx: &mut TestAppContext) {
    let (capturing, writer) = capture();
    let sender = serve(
        cx,
        "__inca_dev__.receive = () => {}; await new Promise(() => {});",
        TEST_ENTRY_PATH,
        &writer,
    );
    cx.run_until_parked();

    cx.executor().advance_clock(Duration::from_millis(1500));
    notify(&sender, "noise");
    cx.run_until_parked();
    assert!(the_window(cx).is_none());

    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    assert_eq!(methods(&capturing), ["ready"]);
    assert!(the_window(cx).is_some());
}

#[gpui::test]
fn a_listener_error_of_a_dispatch_while_pending_is_one_app_error_before_the_window(
    cx: &mut TestAppContext,
) {
    let (capturing, writer) = capture();
    let source = "const n = __inca_native__.createNode('div');\n\
        __inca_native__.addEventListener(n, 'ping');\n\
        globalThis.__inca_callbacks__ = { 0() { throw new Error('x'); } };\n\
        __inca_native__.dispatchEvent(n, new Event('ping'));\n\
        await new Promise(() => {});";
    serve(cx, source, TEST_ENTRY_PATH, &writer);
    cx.run_until_parked();

    assert_eq!(methods(&capturing), ["appError"]);
    assert!(the_window(cx).is_none());
}

#[gpui::test]
fn a_job_failure_while_pending_is_one_app_error_before_the_window(cx: &mut TestAppContext) {
    let (capturing, writer) = capture();
    let source = format!(
        "Promise.reject(new Error('job failed'));\n{}",
        waits_then_mounts(300, 150)
    );
    let sender = serve(cx, &source, TEST_ENTRY_PATH, &writer);
    cx.run_until_parked();

    assert_eq!(methods(&capturing), ["appError"]);
    assert!(the_window(cx).is_none());

    notify(&sender, "go");
    cx.run_until_parked();
    assert_eq!(methods(&capturing), ["appError", "ready"]);
}

#[gpui::test]
fn a_rejection_after_the_timeout_is_one_app_error_and_the_window_stays(cx: &mut TestAppContext) {
    let (capturing, writer) = capture();
    let sender = serve(
        cx,
        "await new Promise((_, reject) => { \
             __inca_dev__.receive = () => reject(new Error('late')); });",
        TEST_ENTRY_PATH,
        &writer,
    );
    cx.run_until_parked();
    cx.executor().advance_clock(LOAD_TIMEOUT);
    cx.run_until_parked();
    assert_eq!(methods(&capturing), ["ready"]);

    notify(&sender, "go");
    cx.run_until_parked();

    assert_eq!(methods(&capturing), ["ready", "appError"]);
    assert!(the_window(cx).is_some());
}

#[gpui::test]
fn shutdown_while_loading_is_answered_and_no_window_opens(cx: &mut TestAppContext) {
    let (capturing, writer) = capture();
    let sender = serve(cx, NEVER_SETTLES, TEST_ENTRY_PATH, &writer);

    request(&sender, 1, "shutdown");
    request(&sender, 2, "nope");
    cx.run_until_parked();

    assert_eq!(
        written(&capturing),
        [json!({ "jsonrpc": "2.0", "id": 1, "result": null })]
    );
    assert!(the_window(cx).is_none());
    assert_eq!(sender.receiver_count(), 0, "the loop ended");
}

#[gpui::test]
fn a_closed_channel_while_loading_ends_the_loop(cx: &mut TestAppContext) {
    let (capturing, writer) = capture();
    let sender = serve(cx, NEVER_SETTLES, TEST_ENTRY_PATH, &writer);
    cx.run_until_parked();

    sender.close();
    cx.run_until_parked();

    assert_eq!(sender.receiver_count(), 0, "the loop ended");
    assert!(capturing.0.borrow().is_empty());
}

/// An entry that settles on `go`, reports a failed load on `fail` and
/// clears the report on `clear`.
fn reports_load_failures() -> String {
    format!(
        "await new Promise((r) => {{ __inca_dev__.receive = (m) => {{ \
             if (m === 'fail') __inca_dev__.setLoadFailed(true); \
             if (m === 'clear') __inca_dev__.setLoadFailed(false); \
             if (m === 'go') r(); }}; }});\n{}",
        mounts_a_div(300, 150)
    )
}

#[gpui::test]
fn a_failed_load_report_keeps_the_window_closed_past_the_timeout(cx: &mut TestAppContext) {
    let (capturing, writer) = capture();
    let sender = serve(cx, &reports_load_failures(), TEST_ENTRY_PATH, &writer);
    notify(&sender, "fail");
    cx.run_until_parked();

    cx.executor().advance_clock(LOAD_TIMEOUT * 3);
    cx.run_until_parked();
    assert!(the_window(cx).is_none());
    assert!(capturing.0.borrow().is_empty());

    notify(&sender, "go");
    cx.run_until_parked();
    assert_eq!(methods(&capturing), ["ready"]);
    let window = the_window(cx).unwrap();
    assert_eq!(bounds_of(cx, window), (300.0, 150.0));
}

#[gpui::test]
fn clearing_the_failed_load_report_restarts_the_timeout(cx: &mut TestAppContext) {
    let (capturing, writer) = capture();
    let sender = serve(cx, &reports_load_failures(), TEST_ENTRY_PATH, &writer);
    notify(&sender, "fail");
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(10));
    cx.run_until_parked();
    assert!(the_window(cx).is_none());

    notify(&sender, "clear");
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(1900));
    cx.run_until_parked();
    assert!(the_window(cx).is_none());

    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    assert_eq!(methods(&capturing), ["ready"]);
    let window = the_window(cx).unwrap();
    assert_eq!(bounds_of(cx, window), (800.0, 600.0));
}

#[gpui::test]
fn a_report_cleared_within_one_poll_restarts_the_timeout(cx: &mut TestAppContext) {
    let (capturing, writer) = capture();
    let sender = serve(
        cx,
        "__inca_dev__.receive = () => { \
             __inca_dev__.setLoadFailed(true); __inca_dev__.setLoadFailed(false); }; \
         await new Promise(() => {});",
        TEST_ENTRY_PATH,
        &writer,
    );
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(1500));
    notify(&sender, "blink");
    cx.run_until_parked();

    cx.executor().advance_clock(Duration::from_millis(1900));
    cx.run_until_parked();
    assert!(the_window(cx).is_none());

    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    assert_eq!(methods(&capturing), ["ready"]);
}

/// Serves `source` until 1 ms before the deadline, and returns the loop's
/// sender with an engine handle for queueing jobs that no poll has drained.
fn serve_with_engine_handle(
    cx: &mut TestAppContext,
    source: &str,
    writer: &SharedWriter,
) -> (async_channel::Sender<String>, Rc<inca_jsenv::Engine>) {
    let started = cx.executor().now();
    let first =
        Session::begin(TEST_ENTRY_PATH, source, reporter_for(writer), Some(writer)).unwrap();
    let engine = Rc::clone(&first.session().engine);
    let sender = serve_started(cx, first, started, TEST_ENTRY_PATH, writer);
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(1999));
    cx.run_until_parked();
    (sender, engine)
}

#[gpui::test]
fn a_failed_load_report_made_by_the_final_poll_keeps_the_window_closed(cx: &mut TestAppContext) {
    let (capturing, writer) = capture();
    let (_sender, engine) = serve_with_engine_handle(cx, NEVER_SETTLES, &writer);
    engine
        .eval::<()>("Promise.resolve().then(() => __inca_dev__.setLoadFailed(true));")
        .unwrap();

    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();

    assert!(the_window(cx).is_none());
    assert!(capturing.0.borrow().is_empty());
}

#[gpui::test]
fn a_report_cleared_by_the_final_poll_restarts_the_timeout(cx: &mut TestAppContext) {
    let (capturing, writer) = capture();
    let (_sender, engine) = serve_with_engine_handle(cx, NEVER_SETTLES, &writer);
    engine
        .eval::<()>(
            "Promise.resolve().then(() => { \
                 __inca_dev__.setLoadFailed(true); __inca_dev__.setLoadFailed(false); });",
        )
        .unwrap();

    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    assert!(the_window(cx).is_none());
    assert!(capturing.0.borrow().is_empty());

    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    assert!(the_window(cx).is_some());
    assert_eq!(written(&capturing), [ready_message()]);
}

#[gpui::test]
fn an_entry_that_settles_at_the_deadline_opens_one_window(cx: &mut TestAppContext) {
    let (capturing, writer) = capture();
    let (_sender, engine) = serve_with_engine_handle(
        cx,
        "await new Promise((r) => { globalThis.release = r; });",
        &writer,
    );
    engine.eval::<()>("release();").unwrap();

    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();

    assert!(the_window(cx).is_some());
    assert_eq!(written(&capturing), [ready_message()]);
}

#[gpui::test]
fn the_deadline_counts_from_the_start_of_the_evaluation(cx: &mut TestAppContext) {
    let (capturing, writer) = capture();
    let started = cx.executor().now();
    let first = Session::begin(
        TEST_ENTRY_PATH,
        NEVER_SETTLES,
        reporter_for(&writer),
        Some(&writer),
    )
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(1500));
    let sender = serve_started(cx, first, started, TEST_ENTRY_PATH, &writer);
    cx.run_until_parked();
    assert!(the_window(cx).is_none());

    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();

    assert!(the_window(cx).is_some());
    assert_eq!(written(&capturing), [ready_message()]);
    assert_eq!(sender.receiver_count(), 1);
}

#[gpui::test]
fn the_timeout_opens_the_window_at_the_size_of_content_already_mounted(cx: &mut TestAppContext) {
    let (capturing, writer) = capture();
    let source = format!("{}\n{NEVER_SETTLES}", mounts_a_div(300, 150));
    let _sender = serve(cx, &source, TEST_ENTRY_PATH, &writer);
    cx.run_until_parked();

    cx.executor().advance_clock(LOAD_TIMEOUT);
    cx.run_until_parked();

    let window = the_window(cx).unwrap();
    assert_eq!(bounds_of(cx, window), (300.0, 150.0));
    assert_eq!(written(&capturing), [ready_message()]);
}

#[gpui::test]
fn a_receive_that_throws_while_loading_is_reported(cx: &mut TestAppContext) {
    let (capturing, writer) = capture();
    let sender = serve(
        cx,
        &format!("__inca_dev__.receive = () => {{ throw new Error('rx'); }};\n{NEVER_SETTLES}"),
        TEST_ENTRY_PATH,
        &writer,
    );

    notify(&sender, "probe");
    cx.run_until_parked();

    let sent = written(&capturing);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["method"], "appError");
    assert_eq!(sent[0]["params"]["message"], "Error: rx");
    assert!(the_window(cx).is_none());
}

#[gpui::test]
fn shutdown_ends_a_held_first_load(cx: &mut TestAppContext) {
    let (capturing, writer) = capture();
    let sender = serve(
        cx,
        "__inca_dev__.setLoadFailed(true); await new Promise(() => {});",
        TEST_ENTRY_PATH,
        &writer,
    );

    request(&sender, 1, "shutdown");
    cx.run_until_parked();

    assert_eq!(
        written(&capturing),
        [json!({ "jsonrpc": "2.0", "id": 1, "result": null })]
    );
    assert_eq!(sender.receiver_count(), 0);
    assert!(the_window(cx).is_none());
}
