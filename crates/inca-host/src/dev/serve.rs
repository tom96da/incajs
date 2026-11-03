// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Serves the development protocol: the lines on stdin, the load in flight and the window.

use std::ops::ControlFlow;
use std::time::{Duration, Instant};

use gpui::{App, FutureExt, WindowHandle};
use serde_json::Value;

use inca_jsenv::{Clock, Engine, ErrorReporter};

use super::reload::params_json;
use super::wire::{self, Incoming, Method, Outgoing};
use super::{ErrorCode, SharedWriter, call_dev_receive, reporter_for, send};
use crate::app::{Failure, HostedApp, Loading, clock_of, maybe_auto_resize_to_content};

/// Answers the request `id` came from. A notification carries no id and
/// takes no reply.
pub(super) fn respond(id: Option<&Value>, outcome: &Result<(), Failure>, writer: &SharedWriter) {
    let Some(id) = id else { return };
    send(
        &**writer,
        &match outcome {
            Ok(()) => Outgoing::result(id.clone()),
            Err(failure) => outgoing(id.clone(), failure),
        },
    );
}

/// The error message that reports `failure` for the request `id`.
pub(super) fn outgoing(id: Value, failure: &Failure) -> Outgoing {
    match failure {
        Failure::Message(code, message) => Outgoing::error(id, *code, message.clone()),
        Failure::Thrown(err) => Outgoing::thrown(id, ErrorCode::BundleFailed, err),
    }
}

/// How long the first load may stay pending with no window before the window
/// opens at the size the config gives, else the mounted content's size, else
/// the default size.
pub(crate) const LOAD_TIMEOUT: Duration = Duration::from_secs(2);

/// Relays an unrecognized notification (no `id`) into the window's app via
/// [`crate::dev::call_dev_receive`], then drains the job queue and
/// refreshes the window as a swap does. A callback may have queued
/// reactivity work or thrown.
fn relay_to_js(
    window: &WindowHandle<HostedApp>,
    cx: &mut App,
    method: &str,
    params: Option<Value>,
    reporter: &ErrorReporter,
) {
    let params_json = params_json(params);

    let _ = window.update(cx, |app, window, cx| {
        app.session.dispatcher.observe_focus(window, cx);
        deliver(&app.session.engine, method, &params_json, reporter);
        app.session.dispatcher.drain_jobs_and_refresh(window, cx);
        maybe_auto_resize_to_content(app, window);
    });
}

/// Calls `__inca_dev__.receive` on `engine` and reports what it throws.
fn deliver(engine: &Engine, method: &str, params_json: &str, reporter: &ErrorReporter) {
    if let Some(err) = call_dev_receive(engine, method, params_json) {
        reporter(&err);
    }
}

/// An entry that is still evaluating, with the `reload` ids waiting on it.
pub(super) struct Pending {
    pub(super) loading: Loading,
    pub(super) ids: Vec<Value>,
}

/// What the serve loop knows: whether a window exists, the load in flight
/// and when the first load's wait ends.
pub(super) struct Serve {
    pub(super) window: Option<WindowHandle<HostedApp>>,
    pub(super) pending: Option<Pending>,
    pub(super) deadline: Instant,
    was_held: bool,
    pub(super) entry_path: String,
    pub(super) writer: SharedWriter,
    pub(super) reporter: ErrorReporter,
    pub(super) clock: Clock,
}

impl Serve {
    /// Whether the first load waits for the app: no window yet and the app
    /// reported a failed load.
    pub(super) fn held(&self) -> bool {
        self.window.is_none()
            && self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.loading.start_failed())
    }

    /// Restarts the deadline when the app clears a failed-load report, even
    /// when the report and the clearing happened between two polls.
    pub(super) fn track_hold(&mut self, cx: &App) {
        let held = self.held();
        let reported = self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.loading.reported_failure());
        if (self.was_held || reported) && !held {
            self.deadline = cx.background_executor().now() + LOAD_TIMEOUT;
        }
        self.was_held = held;
    }

    /// How long to wait for a line: until the load deadline while no window is
    /// open, and until the next timer of the load in flight. `None` waits for
    /// the next line.
    fn wait(&self, cx: &gpui::AsyncApp) -> Option<Duration> {
        let load = (self.window.is_none() && !self.held()).then(|| {
            self.deadline
                .saturating_duration_since(cx.background_executor().now())
        });
        let timer = self
            .pending
            .as_ref()
            .and_then(|pending| pending.loading.session().dispatcher.next_timer_wait());
        match (load, timer) {
            (Some(load), Some(timer)) => Some(load.min(timer)),
            (load, timer) => load.or(timer),
        }
    }

    pub(super) fn answer(&self, ids: &[Value], outcome: &Result<(), Failure>) {
        for id in ids {
            respond(Some(id), outcome, &self.writer);
        }
    }

    /// Relays a notification to the load in flight, or to the window's app
    /// when none is.
    fn relay(&mut self, cx: &mut App, method: &str, params: Option<Value>) {
        if let Some(pending) = &self.pending {
            let params_json = params_json(params);
            let engine = &pending.loading.session().engine;
            deliver(engine, method, &params_json, &self.reporter);
            self.advance(cx);
        } else if let Some(window) = &self.window {
            relay_to_js(window, cx, method, params, &self.reporter);
        }
    }

    /// Handles one stdin line.
    fn on_line(&mut self, cx: &mut App, line: &str) -> ControlFlow<()> {
        match wire::decode(line) {
            Ok(Incoming::Call {
                id,
                method: Method::Reload,
            }) => self.reload(cx, id),
            Ok(Incoming::Call {
                id,
                method: Method::Shutdown,
            }) => {
                respond(id.as_ref(), &Ok(()), &self.writer);
                return ControlFlow::Break(());
            }
            Ok(Incoming::Unrecognized {
                id: Some(id),
                method,
                ..
            }) => respond(
                Some(&id),
                &Err(Failure::Message(
                    ErrorCode::MethodNotFound,
                    format!("unknown method: {method}"),
                )),
                &self.writer,
            ),
            Ok(Incoming::Unrecognized {
                id: None,
                method,
                params,
            }) => self.relay(cx, &method, params),
            Ok(Incoming::Empty) => {}
            Err((code, message)) => {
                send(&*self.writer, &Outgoing::error(Value::Null, code, message));
            }
        }
        ControlFlow::Continue(())
    }
}

/// Starts serving the protocol messages arriving on `lines` while `first`
/// evaluates, and returns once the first poll has run. The loop task answers
/// until `shutdown`, or until the channel closes, then quits the app.
///
/// The window opens when `first` settles, or [`LOAD_TIMEOUT`] after `started`
/// (the instant just before `first` began evaluating) when it is still
/// pending, unless the app reported a failed load. A `first` that fails with
/// no window open ends the process.
pub(crate) fn serve_dev_protocol(
    cx: &mut App,
    lines: async_channel::Receiver<String>,
    first: Loading,
    started: Instant,
    entry_path: String,
    writer: SharedWriter,
) {
    let mut serve = Serve {
        window: None,
        pending: Some(Pending {
            loading: first,
            ids: Vec::new(),
        }),
        deadline: started + LOAD_TIMEOUT,
        was_held: false,
        entry_path,
        reporter: reporter_for(&writer),
        writer,
        clock: clock_of(cx.background_executor()),
    };
    serve.track_hold(cx);
    serve.advance(cx);

    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        loop {
            let received = match serve.wait(cx) {
                Some(wait) => lines
                    .recv()
                    .with_timeout(wait, cx.background_executor())
                    .await
                    .ok(),
                None => Some(lines.recv().await),
            };
            let flow = match received {
                Some(Ok(line)) => cx.update(|cx| serve.on_line(cx, &line)),
                Some(Err(_)) => ControlFlow::Break(()),
                None => {
                    cx.update(|cx| serve.time_out(cx));
                    ControlFlow::Continue(())
                }
            };
            if flow.is_break() {
                break;
            }
        }
        cx.update(|cx| cx.quit());
    })
    .detach();
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use gpui::TestAppContext;
    use inca_bridge::test_support::TIMERS_JS;
    use serde_json::json;

    use crate::tests::{
        RECEIVES_DEV_EVENTS, ScratchDir, TEST_ENTRY_PATH, capture, eval_in_window, line, methods,
        notify, request, serve_idle, serve_idle_on_executor_clock, serve_on_executor_clock,
        the_window, write_entry, written,
    };

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
        let (window, sender) =
            serve_idle(cx, THROWS_AFTER_QUEUEING_A_JOB, TEST_ENTRY_PATH, &capturing);

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
        assert!(
            written(&capturing).contains(&json!({ "jsonrpc": "2.0", "id": 5, "result": null }))
        );
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
}
