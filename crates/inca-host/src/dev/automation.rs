// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The automation requests of the development protocol: `click`, `snapshot` and `waitIdle`.

use gpui::{
    AnyWindowHandle, App, InputEvent, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, WindowHandle,
};
use inca_node::NodeId;
use serde::Deserialize;
use serde_json::Value;

use super::ErrorCode;
use super::serve::{Serve, reply};
use super::wire::Automation;
use crate::app::{Failure, HostedApp};
use crate::snapshot::{Bounds, Lookup, Node, snapshot};

const NO_WINDOW: &str = "no window is open";

impl Serve {
    /// Runs an automation request and answers it.
    pub(super) fn automate(
        &mut self,
        cx: &mut App,
        id: Option<&Value>,
        request: Automation,
        params: Option<Value>,
    ) {
        let outcome = match self.window {
            None => Err(Failure::Message(
                ErrorCode::Unavailable,
                NO_WINDOW.to_owned(),
            )),
            Some(window) => match request {
                Automation::Click => click(window, cx, params),
                Automation::Snapshot => snapshot_of(window, cx)
                    .map(|root| serde_json::to_value(root).expect("a snapshot serializes")),
                Automation::WaitIdle => settle(window, cx).map(|()| Value::Null),
            },
        };
        match outcome {
            Ok(value) => reply(id, Ok(value), &self.writer),
            Err(failure) => reply(id, Err(&failure), &self.writer),
        }
    }
}

/// The failure of a window update on a closed window. The update's error says nothing more, so
/// it is dropped.
fn closed(_: impl std::fmt::Display) -> Failure {
    Failure::Message(ErrorCode::Unavailable, NO_WINDOW.to_owned())
}

/// Runs the queued jobs and the frame callbacks requested so far, then draws a frame of the
/// current tree, the one hit tests and snapshots read.
fn settle(window: WindowHandle<HostedApp>, cx: &mut App) -> Result<(), Failure> {
    window
        .update(cx, |app, window, cx| {
            app.session.dispatcher.drain_jobs_and_refresh(window, cx);
            if app.session.host.js.borrow().frames.is_pending() {
                app.session.dispatcher.run_frame_callbacks(window, cx);
            }
        })
        .map_err(closed)?;
    AnyWindowHandle::from(window)
        .update(cx, |_, window, cx| window.draw(cx).clear(cx))
        .map_err(closed)
}

/// The app's tree with the boxes of the last drawn frame.
fn snapshot_of(window: WindowHandle<HostedApp>, cx: &mut App) -> Result<Node, Failure> {
    window
        .update(cx, |app, _, _| {
            let document = app.session.host.document.borrow();
            let window = app.session.host.window.borrow();
            snapshot(
                &document.tree,
                &document.style,
                document.root(),
                &mut |id| {
                    window
                        .layout_of(id)
                        .map(|layout| Bounds::from(layout.bounds))
                },
            )
            .expect("the host declares a root")
        })
        .map_err(closed)
}

/// The `params` of `click`.
#[derive(Deserialize)]
struct ClickParams {
    node: NodeId,
}

/// Moves the pointer to the center of a node's box, draws, then presses and releases the primary
/// button at that point.
fn click(
    window: WindowHandle<HostedApp>,
    cx: &mut App,
    params: Option<Value>,
) -> Result<Value, Failure> {
    let ClickParams { node } = serde_json::from_value(params.unwrap_or(Value::Null))
        .map_err(|err| invalid(format!("invalid params: {err}")))?;
    settle(window, cx)?;
    let position = match snapshot_of(window, cx)?.nearest_bounds(node) {
        Lookup::At(bounds) => bounds.center(),
        Lookup::Absent => return Err(invalid(format!("no node {node} in the window"))),
        Lookup::Unbounded => {
            return Err(Failure::Message(
                ErrorCode::Unavailable,
                format!("node {node} and its ancestors have no box"),
            ));
        }
    };
    let any = AnyWindowHandle::from(window);
    let moved = || {
        MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers: Modifiers::none(),
        }
        .to_platform_input()
    };
    any.update(cx, |_, window, cx| {
        window.dispatch_event(moved(), cx);
    })
    .map_err(closed)?;
    settle(window, cx)?;
    // The draw above can change which node the pointer is over. A second move reports that
    // change before the press.
    any.update(cx, |_, window, cx| {
        window.dispatch_event(moved(), cx);
        let pressed = MouseDownEvent {
            position,
            button: MouseButton::Left,
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        };
        window.dispatch_event(pressed.to_platform_input(), cx);
        let released = MouseUpEvent {
            position,
            button: MouseButton::Left,
            modifiers: Modifiers::none(),
            click_count: 1,
        };
        window.dispatch_event(released.to_platform_input(), cx);
    })
    .map_err(closed)?;
    Ok(Value::Null)
}

fn invalid(message: String) -> Failure {
    Failure::Message(ErrorCode::InvalidParams, message)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use gpui::{TestAppContext, WindowHandle};
    use inca_node::NodeId;
    use inca_test_macros::gpui_cases;
    use serde_json::{Value, json};

    use crate::app::HostedApp;
    use crate::tests::{
        NEVER_SETTLES, RECEIVES_DEV_EVENTS, ScratchDir, capture, eval_in_window, mounts_a_div,
        notify_with, request_with, serve_idle, serve_on_executor_clock, write_entry, written,
    };

    /// A 100 x 50 box at (40, 20) whose center is (90, 45), a `<style>` element beside it, and a
    /// log of the pointer events the box receives.
    const SCENE: &str = r"
        const n = __inca_native__;
        const box = n.createNode('div');
        n.setStyle(box, 'width', 100);
        n.setStyle(box, 'height', 50);
        n.setStyle(box, 'margin_left', 40);
        n.setStyle(box, 'margin_top', 20);
        n.appendChild(n.rootNodeId(), box);
        const style = n.createNode('style');
        n.appendChild(n.rootNodeId(), style);
        globalThis.box = box;
        globalThis.sty = style;
        globalThis.log = [];
        for (const type of ['mouseover', 'mousedown', 'mouseup', 'click']) {
            n.addEventListener(box, type, (e) => {
                globalThis.log.push(e.type + '@' + e.clientX + '/' + e.clientY);
            });
        }
    ";

    const CLICKED_AT_THE_CENTER: &str = "mouseover@90/45,mousedown@90/45,mouseup@90/45,click@90/45";

    fn id_of(cx: &mut TestAppContext, window: WindowHandle<HostedApp>, name: &str) -> NodeId {
        eval_in_window(cx, window, &format!("String(globalThis.{name});"))
            .parse()
            .unwrap()
    }

    fn log_of(cx: &mut TestAppContext, window: WindowHandle<HostedApp>) -> String {
        eval_in_window(cx, window, "globalThis.log.join(',');")
    }

    fn null_result(id: u32) -> Value {
        json!({ "jsonrpc": "2.0", "id": id, "result": null })
    }

    fn failure(id: u32, code: i32, message: &str) -> Value {
        json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
    }

    #[gpui::test]
    fn click_sends_a_move_a_press_and_a_release_to_the_node(cx: &mut TestAppContext) {
        let (capturing, _) = capture();
        let (window, sender) = serve_idle(cx, SCENE, "/test/entry.js", &capturing);
        let node = id_of(cx, window, "box");

        request_with(&sender, 1, "click", &json!({ "node": node }));
        cx.run_until_parked();

        assert_eq!(log_of(cx, window), CLICKED_AT_THE_CENTER);
    }

    #[gpui::test]
    fn click_on_a_text_node_lands_on_its_parent(cx: &mut TestAppContext) {
        let (capturing, _) = capture();
        let source = format!(
            r"
            {SCENE}
            const label = n.createNode('#text');
            n.setCharacterData(label, 'hi');
            n.appendChild(box, label);
            globalThis.label = label;
            "
        );
        let (window, sender) = serve_idle(cx, &source, "/test/entry.js", &capturing);
        let node = id_of(cx, window, "label");

        request_with(&sender, 1, "click", &json!({ "node": node }));
        cx.run_until_parked();

        assert_eq!(log_of(cx, window), CLICKED_AT_THE_CENTER);
    }

    #[gpui::test]
    fn click_draws_between_the_move_and_the_press(cx: &mut TestAppContext) {
        let (capturing, _) = capture();
        let source = r"
            const n = __inca_native__;
            const parent = n.createNode('div');
            n.setStyle(parent, 'width', 400);
            n.setStyle(parent, 'height', 200);
            n.setStyle(parent, 'margin_top', 10);
            n.setStyle(parent, 'margin_left', 10);
            n.appendChild(n.rootNodeId(), parent);
            const box = n.createNode('div');
            n.setStyle(box, 'width', 100);
            n.setStyle(box, 'height', 50);
            n.setStyle(box, 'margin_top', 20);
            n.setStyle(box, 'margin_bottom', 20);
            n.setStyle(box, 'margin_left', 40);
            n.setStyle(box, 'margin_right', 40);
            n.appendChild(parent, box);
            const names = { [box]: 'box', [parent]: 'parent' };
            globalThis.box = box;
            globalThis.log = [];
            n.addEventListener(box, 'mouseover', () => {
                n.setStyle(box, 'display', 'none');
            });
            for (const type of ['mouseover', 'mousedown', 'mouseup', 'click']) {
                for (const node of [box, parent]) {
                    n.addEventListener(node, type, (e) => {
                        globalThis.log.push(
                            e.type + '@' + names[e.currentTarget] + ':' + names[e.target]
                        );
                    });
                }
            }
        ";
        let (window, sender) = serve_idle(cx, source, "/test/entry.js", &capturing);
        let node = id_of(cx, window, "box");

        request_with(&sender, 1, "click", &json!({ "node": node }));
        cx.run_until_parked();

        assert_eq!(
            log_of(cx, window),
            [
                "mouseover@box:box",
                "mouseover@parent:box",
                "mouseover@parent:parent",
                "mousedown@parent:parent",
                "mouseup@parent:parent",
                "click@parent:parent",
            ]
            .join(",")
        );
    }

    #[gpui::test]
    fn click_reaches_a_node_added_since_the_last_draw(cx: &mut TestAppContext) {
        let (capturing, _) = capture();
        let (window, sender) = serve_idle(cx, "", "/test/entry.js", &capturing);
        eval_in_window(
            cx,
            window,
            r"
            const n = __inca_native__;
            const box = n.createNode('div');
            n.setStyle(box, 'width', 100);
            n.setStyle(box, 'height', 50);
            n.appendChild(n.rootNodeId(), box);
            globalThis.box = box;
            globalThis.log = [];
            n.addEventListener(box, 'click', () => {
                globalThis.log.push('click');
            });
            '';
            ",
        );
        let node = id_of(cx, window, "box");

        request_with(&sender, 1, "click", &json!({ "node": node }));
        cx.run_until_parked();

        assert_eq!(log_of(cx, window), "click");
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::unknown_id("999")]
    #[case::style_child("globalThis.sty")]
    fn click_on_a_node_outside_the_snapshot_answers_invalid_params(
        cx: &mut TestAppContext,
        #[case] node: &str,
    ) {
        let (capturing, _) = capture();
        let (window, sender) = serve_idle(cx, SCENE, "/test/entry.js", &capturing);
        let node: NodeId = eval_in_window(cx, window, &format!("String({node});"))
            .parse()
            .unwrap();

        request_with(&sender, 1, "click", &json!({ "node": node }));
        cx.run_until_parked();

        assert_eq!(
            written(&capturing),
            [failure(1, -32602, &format!("no node {node} in the window"))]
        );
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::no_params(Value::Null)]
    #[case::empty_object(json!({}))]
    #[case::string_id(json!({ "node": "3" }))]
    fn click_with_bad_params_answers_invalid_params(
        cx: &mut TestAppContext,
        #[case] params: Value,
    ) {
        let (capturing, _) = capture();
        let (_, sender) = serve_idle(cx, SCENE, "/test/entry.js", &capturing);

        request_with(&sender, 1, "click", &params);
        cx.run_until_parked();

        let sent = written(&capturing);
        assert_eq!(sent[0]["error"]["code"], -32602);
        assert!(
            sent[0]["error"]["message"]
                .as_str()
                .unwrap()
                .starts_with("invalid params:")
        );
    }

    #[gpui::test]
    fn click_on_a_node_with_no_box_answers_unavailable(cx: &mut TestAppContext) {
        let (capturing, _) = capture();
        let (window, sender) = serve_idle(cx, SCENE, "/test/entry.js", &capturing);
        eval_in_window(
            cx,
            window,
            r"
            const n = __inca_native__;
            n.removeChild(n.documentNodeId(), n.rootNodeId());
            globalThis.top = n.rootNodeId();
            '';
            ",
        );
        let node = id_of(cx, window, "top");

        request_with(&sender, 1, "click", &json!({ "node": node }));
        cx.run_until_parked();

        assert_eq!(
            written(&capturing),
            [failure(
                1,
                -32001,
                &format!("node {node} and its ancestors have no box")
            )]
        );
    }

    #[gpui::test]
    fn an_automation_request_before_the_window_opens_answers_unavailable(cx: &mut TestAppContext) {
        let (capturing, writer) = capture();
        let sender = serve_on_executor_clock(cx, NEVER_SETTLES, "/test/entry.js", &writer);
        cx.run_until_parked();

        request_with(&sender, 1, "click", &json!({}));
        cx.run_until_parked();

        assert_eq!(
            written(&capturing),
            [failure(1, -32001, "no window is open")]
        );
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::wait_idle("waitIdle")]
    #[case::snapshot("snapshot")]
    fn an_automation_request_after_the_window_closes_answers_unavailable(
        cx: &mut TestAppContext,
        #[case] method: &str,
    ) {
        let (capturing, _) = capture();
        let (window, sender) = serve_idle(cx, "", "/test/entry.js", &capturing);
        cx.update(|cx| {
            window
                .update(cx, |_, window, _| window.remove_window())
                .unwrap();
        });

        request_with(&sender, 1, method, &Value::Null);
        cx.run_until_parked();

        assert_eq!(
            written(&capturing),
            [failure(1, -32001, "no window is open")]
        );
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::click("click", "{ node: globalThis.box }")]
    #[case::wait_idle("waitIdle", "null")]
    fn a_void_automation_request_answers_null(
        cx: &mut TestAppContext,
        #[case] method: &str,
        #[case] params: &str,
    ) {
        let (capturing, _) = capture();
        let (window, sender) = serve_idle(cx, SCENE, "/test/entry.js", &capturing);
        let params = eval_in_window(cx, window, &format!("JSON.stringify({params});"));

        request_with(&sender, 1, method, &serde_json::from_str(&params).unwrap());
        cx.run_until_parked();

        assert_eq!(written(&capturing), [null_result(1)]);
    }

    #[gpui::test]
    fn click_ignores_unknown_fields_of_its_params(cx: &mut TestAppContext) {
        let (capturing, _) = capture();
        let (window, sender) = serve_idle(cx, SCENE, "/test/entry.js", &capturing);
        let node = id_of(cx, window, "box");

        request_with(&sender, 1, "click", &json!({ "node": node, "extra": 1 }));
        cx.run_until_parked();

        assert_eq!(log_of(cx, window), CLICKED_AT_THE_CENTER);
    }

    #[gpui::test]
    fn an_automation_notification_runs_without_a_reply(cx: &mut TestAppContext) {
        let (capturing, _) = capture();
        let (window, sender) = serve_idle(cx, SCENE, "/test/entry.js", &capturing);
        let node = id_of(cx, window, "box");

        notify_with(&sender, "click", &json!({ "node": node }));
        cx.run_until_parked();

        assert_eq!(log_of(cx, window), CLICKED_AT_THE_CENTER);
        assert!(written(&capturing).is_empty());
    }

    #[gpui::test]
    fn an_automation_notification_is_not_relayed_into_the_app(cx: &mut TestAppContext) {
        let (capturing, _) = capture();
        let source = format!("{RECEIVES_DEV_EVENTS}{SCENE}");
        let (window, sender) = serve_idle(cx, &source, "/test/entry.js", &capturing);
        let node = id_of(cx, window, "box");

        notify_with(&sender, "click", &json!({ "node": node }));
        cx.run_until_parked();

        assert_eq!(log_of(cx, window), CLICKED_AT_THE_CENTER);
        assert_eq!(
            eval_in_window(cx, window, "JSON.stringify(globalThis.received);"),
            "[]"
        );
    }

    #[gpui::test]
    fn snapshot_answers_the_tree_of_the_window(cx: &mut TestAppContext) {
        let (capturing, _) = capture();
        let source = r"
            const n = __inca_native__;
            const box = n.createNode('div');
            n.setAttribute(box, 'class', 'box');
            n.appendChild(n.rootNodeId(), box);
            const label = n.createNode('#text');
            n.setCharacterData(label, 'hi');
            n.appendChild(box, label);
            const style = n.createNode('style');
            n.appendChild(n.rootNodeId(), style);
            const template = n.createNode('template');
            n.appendChild(n.rootNodeId(), template);
            n.appendChild(template, n.createNode('div'));
            const comment = n.createNode('#comment');
            n.setCharacterData(comment, 'note');
            n.appendChild(n.rootNodeId(), comment);
            globalThis.root = n.rootNodeId();
            globalThis.box = box;
        ";
        let (window, sender) = serve_idle(cx, source, "/test/entry.js", &capturing);
        request_with(&sender, 1, "waitIdle", &Value::Null);
        cx.run_until_parked();
        let rect: Vec<f64> = serde_json::from_str(&eval_in_window(
            cx,
            window,
            "JSON.stringify(__inca_native__.layoutOf(globalThis.box));",
        ))
        .unwrap();
        let (root, box_id) = (id_of(cx, window, "root"), id_of(cx, window, "box"));

        request_with(&sender, 2, "snapshot", &Value::Null);
        cx.run_until_parked();

        let tree = &written(&capturing)[1]["result"];
        assert_eq!(tree["id"], root);
        let [boxed] = &tree["children"].as_array().unwrap()[..] else {
            panic!("the root has one child in the snapshot");
        };
        assert_eq!(boxed["id"], box_id);
        assert_eq!(boxed["tag"], "div");
        assert_eq!(boxed["attributes"], json!({ "class": "box" }));
        assert_eq!(
            boxed["bounds"],
            json!({ "x": rect[0], "y": rect[1], "width": rect[2], "height": rect[3] })
        );
        let text = &boxed["children"][0];
        assert_eq!(
            (
                &text["tag"],
                &text["text"],
                &text["attributes"],
                &text["bounds"]
            ),
            (&json!("#text"), &json!("hi"), &json!({}), &Value::Null)
        );
    }

    #[gpui::test]
    fn snapshot_reports_no_box_for_a_node_added_since_the_last_draw(cx: &mut TestAppContext) {
        let (capturing, _) = capture();
        let (window, sender) = serve_idle(cx, "", "/test/entry.js", &capturing);
        eval_in_window(
            cx,
            window,
            r"
            const n = __inca_native__;
            const box = n.createNode('div');
            n.setStyle(box, 'width', 100);
            n.setStyle(box, 'height', 50);
            n.appendChild(n.rootNodeId(), box);
            '';
            ",
        );

        request_with(&sender, 1, "snapshot", &Value::Null);
        cx.run_until_parked();

        let child = &written(&capturing)[0]["result"]["children"][0];
        assert_eq!(
            (&child["tag"], &child["bounds"]),
            (&json!("div"), &Value::Null)
        );
    }

    #[gpui::test]
    fn wait_idle_runs_the_queued_jobs(cx: &mut TestAppContext) {
        let (capturing, _) = capture();
        let (window, sender) =
            serve_idle(cx, "globalThis.ran = false;", "/test/entry.js", &capturing);
        eval_in_window(
            cx,
            window,
            r"
            Promise.resolve().then(() => {
                globalThis.ran = true;
            });
            '';
            ",
        );

        request_with(&sender, 1, "waitIdle", &Value::Null);
        cx.run_until_parked();

        assert_eq!(
            eval_in_window(cx, window, "String(globalThis.ran);"),
            "true"
        );
    }

    #[gpui::test]
    fn wait_idle_runs_frame_callbacks_requested_before_it(cx: &mut TestAppContext) {
        let (capturing, _) = capture();
        let source = r"
            globalThis.frames = [];
            requestAnimationFrame(() => {
                globalThis.frames.push('outer');
                requestAnimationFrame(() => {
                    globalThis.frames.push('inner');
                });
            });
        ";
        let (window, sender) = serve_idle(cx, source, "/test/entry.js", &capturing);

        request_with(&sender, 1, "waitIdle", &Value::Null);
        cx.run_until_parked();
        let after_the_first = eval_in_window(cx, window, "globalThis.frames.join(',');");
        request_with(&sender, 2, "waitIdle", &Value::Null);
        cx.run_until_parked();
        let after_the_second = eval_in_window(cx, window, "globalThis.frames.join(',');");

        assert_eq!(
            (after_the_first.as_str(), after_the_second.as_str()),
            ("outer", "outer,inner")
        );
    }

    #[gpui::test]
    fn an_automation_request_during_a_pending_reload_reads_the_window(cx: &mut TestAppContext) {
        let dir = ScratchDir::new("automation-reload");
        let source = format!(
            r"
            const n = __inca_native__;
            const span = n.createNode('span');
            n.appendChild(n.rootNodeId(), span);
            {NEVER_SETTLES}
            "
        );
        let entry = write_entry(&dir, &source);
        let (capturing, _) = capture();
        let (_, sender) = serve_idle(cx, &mounts_a_div(300, 150), &entry, &capturing);
        request_with(&sender, 1, "reload", &Value::Null);
        cx.run_until_parked();

        request_with(&sender, 2, "snapshot", &Value::Null);
        cx.run_until_parked();

        let sent = written(&capturing);
        assert_eq!(sent[0]["id"], 2);
        assert_eq!(sent[0]["result"]["children"][0]["tag"], "div");
    }
}
