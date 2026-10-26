// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use inca_bridge::test_support::TIMERS_JS;

use super::*;

#[gpui::test]
fn a_reload_starts_the_pump_of_the_new_session(cx: &mut TestAppContext) {
    let dir = ScratchDir::new("timer-reload");
    let old = [
        TIMERS_JS,
        "setTimeout(() => __inca_dev__.send('old_fired', '{}'), 100);",
    ]
    .concat();
    let entry = write_entry(&dir, &old);
    let (capturing, _) = capture();
    let (window, sender) = serve_idle_on_executor_clock(cx, &old, &entry, &capturing);
    let keep = window
        .update(cx, |app, _, _| app.session.dispatcher.clone())
        .unwrap();

    std::fs::write(
        &entry,
        [
            TIMERS_JS,
            "setTimeout(() => __inca_dev__.send('new_fired', '{}'), 100);",
        ]
        .concat(),
    )
    .unwrap();
    request(&sender, 1, "reload");
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();

    let sent = methods(&capturing);
    assert!(sent.contains(&"new_fired".to_owned()));
    assert!(!sent.contains(&"old_fired".to_owned()));
    drop(keep);
}

#[gpui::test]
fn a_reload_that_awaits_a_timer_settles_when_the_clock_reaches_it(cx: &mut TestAppContext) {
    let dir = ScratchDir::new("timer-awaiting-reload");
    let entry = write_entry(&dir, "");
    let (capturing, _) = capture();
    let (_window, sender) = serve_idle_on_executor_clock(cx, "", &entry, &capturing);

    std::fs::write(
        &entry,
        [TIMERS_JS, "await new Promise((r) => setTimeout(r, 500));"].concat(),
    )
    .unwrap();
    request(&sender, 5, "reload");
    cx.run_until_parked();
    assert!(
        written(&capturing)
            .iter()
            .all(|sent| sent["id"] != json!(5))
    );
    cx.executor().advance_clock(Duration::from_millis(499));
    cx.run_until_parked();
    assert!(
        written(&capturing)
            .iter()
            .all(|sent| sent["id"] != json!(5))
    );

    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    assert!(written(&capturing).contains(&json!({ "jsonrpc": "2.0", "id": 5, "result": null })));
}

#[gpui::test]
fn an_entry_that_awaits_a_timer_settles_when_the_clock_reaches_it(cx: &mut TestAppContext) {
    let (capturing, writer) = capture();
    let _sender = serve_on_executor_clock(
        cx,
        &[TIMERS_JS, "await new Promise((r) => setTimeout(r, 500));"].concat(),
        TEST_ENTRY_PATH,
        &writer,
    );
    cx.run_until_parked();
    assert!(the_window(cx).is_none());

    cx.executor().advance_clock(Duration::from_millis(499));
    cx.run_until_parked();
    assert!(the_window(cx).is_none());

    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    assert!(the_window(cx).is_some());
    assert_eq!(methods(&capturing), ["ready"]);
}
