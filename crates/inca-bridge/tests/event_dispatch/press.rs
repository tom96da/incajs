// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;
use inca_test_macros::gpui_cases;

#[gpui::test]
fn mouseup_runs_its_whole_path_before_click(cx: &mut TestAppContext) {
    let (mut cx, engine, parent, child) = mount_recording_pair(cx, &["click", "mouseup"]);

    cx.simulate_click(point(px(10.0), px(10.0)), Modifiers::none());

    let seen = take_ids(&engine);
    let order: Vec<(&str, NodeId)> = seen
        .iter()
        .map(|entry| (entry.0.as_str(), entry.1))
        .collect();
    assert_eq!(
        order,
        [
            ("mouseup", child),
            ("mouseup", parent),
            ("click", child),
            ("click", parent)
        ]
    );
    assert_eq!(seen[0].2, seen[1].2);
    assert_eq!(seen[2].2, seen[3].2);
    assert_ne!(seen[0].2, seen[2].2);
}

/// Mounts a parent over a child, both listening for `events`, recording
/// `[type, target, currentTarget, detail, pointerId]` per event.
fn mount_click_recorder(
    cx: &mut TestAppContext,
    events: &[&str],
) -> (VisualTestContext, Rc<Engine>, NodeId, NodeId) {
    mount_recording_pair_with(
        cx,
        events,
        r"
        (e) => {
            globalThis.seen.push([
                e.type, e.target, e.currentTarget, e.detail, e.pointerId ?? null]
            );
        }
        ",
    )
}

fn take_clicks(engine: &Engine) -> Vec<(String, NodeId, NodeId, u32, Option<i32>)> {
    let seen = engine
        .eval::<String>("JSON.stringify(globalThis.seen.splice(0))")
        .unwrap();
    serde_json::from_str(&seen).unwrap()
}

/// Presses `down` and releases `up`, each a `(button, click_count)`.
fn press_and_release(
    cx: &mut VisualTestContext,
    (down, down_count): (MouseButton, usize),
    (up, up_count): (MouseButton, usize),
) {
    let position = point(px(10.0), px(10.0));
    cx.simulate_event(MouseDownEvent {
        position,
        button: down,
        modifiers: Modifiers::none(),
        click_count: down_count,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        position,
        button: up,
        modifiers: Modifiers::none(),
        click_count: up_count,
    });
}

#[gpui_cases]
#[gpui::test]
#[case::right_button(MouseButton::Right)]
#[case::middle_button(MouseButton::Middle)]
fn another_button_releases_as_auxclick_and_not_click(cx: &mut TestAppContext, button: MouseButton) {
    let (mut cx, engine, parent, child) = mount_click_recorder(cx, &["click", "auxclick"]);

    press_and_release(&mut cx, (button, 1), (button, 1));

    let aux = |target| ("auxclick".to_owned(), child, target, 1, Some(1));
    assert_eq!(take_clicks(&engine), [aux(child), aux(parent)]);
}

#[gpui::test]
fn the_second_click_of_a_double_click_fires_dblclick_with_no_pointer_fields(
    cx: &mut TestAppContext,
) {
    let (mut cx, engine, parent, child) = mount_click_recorder(cx, &["click", "dblclick"]);

    press_and_release(&mut cx, (MouseButton::Left, 1), (MouseButton::Left, 1));
    let single = take_clicks(&engine);
    press_and_release(&mut cx, (MouseButton::Left, 2), (MouseButton::Left, 2));

    let click = |count, target| ("click".to_owned(), child, target, count, Some(1));
    let double = |target| ("dblclick".to_owned(), child, target, 2, None);
    assert_eq!(single, [click(1, child), click(1, parent)]);
    assert_eq!(
        take_clicks(&engine),
        [
            click(2, child),
            click(2, parent),
            double(child),
            double(parent)
        ]
    );
}

#[gpui::test]
fn a_release_reports_the_click_count_of_its_press(cx: &mut TestAppContext) {
    let (mut cx, engine, _parent, _child) =
        mount_click_recorder(cx, &["mouseup", "click", "dblclick"]);

    press_and_release(&mut cx, (MouseButton::Left, 1), (MouseButton::Left, 0));
    let single = take_clicks(&engine);
    press_and_release(&mut cx, (MouseButton::Left, 2), (MouseButton::Left, 0));
    let double = take_clicks(&engine);

    let details = |seen: &[(String, NodeId, NodeId, u32, Option<i32>)]| -> Vec<(String, u32)> {
        seen.iter().map(|c| (c.0.clone(), c.3)).collect()
    };
    let at = |name: &str, count| (name.to_owned(), count);
    assert_eq!(
        details(&single),
        [
            at("mouseup", 1),
            at("mouseup", 1),
            at("click", 1),
            at("click", 1)
        ]
    );
    assert_eq!(
        details(&double),
        [
            at("mouseup", 2),
            at("mouseup", 2),
            at("click", 2),
            at("click", 2),
            at("dblclick", 2),
            at("dblclick", 2)
        ]
    );
}

#[gpui::test]
fn an_auxclick_takes_its_detail_from_the_press(cx: &mut TestAppContext) {
    let (mut cx, engine, _parent, _child) = mount_click_recorder(cx, &["auxclick"]);

    press_and_release(&mut cx, (MouseButton::Right, 2), (MouseButton::Right, 0));

    let counts: Vec<_> = take_clicks(&engine).into_iter().map(|c| c.3).collect();
    assert_eq!(counts, [2, 2]);
}

#[gpui::test]
fn a_release_of_another_button_keeps_its_own_click_count(cx: &mut TestAppContext) {
    let (mut cx, engine, _parent, _child) = mount_click_recorder(cx, &["mouseup"]);

    press_and_release(&mut cx, (MouseButton::Left, 2), (MouseButton::Right, 1));

    let counts: Vec<_> = take_clicks(&engine).into_iter().map(|c| c.3).collect();
    assert_eq!(counts, [1, 1]);
}

#[gpui::test]
fn a_release_with_no_press_keeps_its_own_click_count(cx: &mut TestAppContext) {
    let (mut cx, engine, _parent, _child) = mount_click_recorder(cx, &["mouseup"]);

    cx.simulate_event(MouseUpEvent {
        position: point(px(10.0), px(10.0)),
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 5,
    });

    let counts: Vec<_> = take_clicks(&engine).into_iter().map(|c| c.3).collect();
    assert_eq!(counts, [5, 5]);
}

/// Panes with `events` listened for on the row, recording
/// `[type, target, button]`.
fn mount_target_recorder(cx: &mut TestAppContext, events: &[&str]) -> (VisualTestContext, Panes) {
    let panes = build_panes_recording(
        &[],
        events,
        "(e) => { globalThis.seen.push([e.type, e.target, e.button]); }",
    );
    (mount_panes(cx, &panes), panes)
}

fn take_targets(engine: &Engine) -> Vec<(String, NodeId, u8)> {
    let seen = engine
        .eval::<String>("JSON.stringify(globalThis.seen.splice(0))")
        .unwrap();
    serde_json::from_str(&seen).unwrap()
}

#[gpui::test]
fn a_second_button_press_leaves_the_first_buttons_click(cx: &mut TestAppContext) {
    let (mut cx, panes) = mount_target_recorder(cx, &["click", "auxclick"]);
    let none = Modifiers::none();
    let (in_a, in_b) = (point(px(10.0), px(10.0)), point(px(150.0), px(10.0)));

    cx.simulate_mouse_down(in_a, MouseButton::Left, none);
    cx.simulate_mouse_down(in_b, MouseButton::Right, none);
    cx.simulate_mouse_up(in_a, MouseButton::Left, none);
    let left = take_targets(&panes.engine);
    cx.simulate_mouse_up(in_b, MouseButton::Right, none);

    assert_eq!(left, [("click".to_owned(), panes.a, 0)]);
    assert_eq!(
        take_targets(&panes.engine),
        [("auxclick".to_owned(), panes.b, 2)]
    );
}

#[gpui::test]
fn a_release_beyond_the_tree_clicks_the_common_ancestor_with_the_root(cx: &mut TestAppContext) {
    let (mut cx, panes) = mount_target_recorder(cx, &["click"]);

    cx.simulate_mouse_down(
        point(px(10.0), px(10.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_up(
        point(px(500.0), px(500.0)),
        MouseButton::Left,
        Modifiers::none(),
    );

    assert_eq!(
        take_targets(&panes.engine),
        [("click".to_owned(), panes.root, 0)]
    );
}

#[gpui::test]
fn a_press_and_release_in_two_panes_clicks_their_common_ancestor(cx: &mut TestAppContext) {
    let panes = build_panes_recording(
        &["click"],
        &["click"],
        "(e) => { globalThis.seen.push([e.type, e.target, e.currentTarget]); }",
    );
    let mut cx = mount_panes(cx, &panes);

    cx.simulate_mouse_down(
        point(px(10.0), px(10.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_up(
        point(px(150.0), px(10.0)),
        MouseButton::Left,
        Modifiers::none(),
    );

    let seen = panes
        .engine
        .eval::<String>("JSON.stringify(globalThis.seen.splice(0))")
        .unwrap();
    let seen: Vec<(String, NodeId, NodeId)> = serde_json::from_str(&seen).unwrap();
    assert_eq!(seen, [("click".to_owned(), panes.root, panes.root)]);
}

#[gpui::test]
fn a_third_click_fires_no_dblclick(cx: &mut TestAppContext) {
    let (mut cx, engine, parent, child) = mount_click_recorder(cx, &["click", "dblclick"]);

    press_and_release(&mut cx, (MouseButton::Left, 3), (MouseButton::Left, 3));

    let click = |target| ("click".to_owned(), child, target, 3, Some(1));
    assert_eq!(take_clicks(&engine), [click(child), click(parent)]);
}

#[gpui::test]
fn a_press_ends_when_the_window_deactivates(cx: &mut TestAppContext) {
    let panes = build_panes(&["click"], &[]);
    let mut cx = mount_panes(cx, &panes);
    let position = point(px(50.0), px(50.0));
    panes.engine.eval::<()>("globalThis.seen = [];").unwrap();

    cx.simulate_mouse_down(position, MouseButton::Left, Modifiers::none());
    cx.update(|window, cx| panes.dispatcher.window_activated(false, window, cx));
    cx.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());

    assert_eq!(panes.engine.eval::<u32>("seen.length").unwrap(), 0);
}

#[gpui_cases]
#[gpui::test]
#[case::stopped("e.stopPropagation();")]
#[case::prevented("e.preventDefault();")]
fn a_right_press_fires_contextmenu_after_a_handled_mousedown(
    cx: &mut TestAppContext,
    handle: &str,
) {
    let (host, node) = build_clickable_tree();
    let engine = test_support::engine_for(&host);
    engine
        .eval::<()>(&format!(
            r"
            globalThis.seen = [];
            __inca_test_listen__({node}, 'mousedown', (e) => {{
                {handle}
            }});
            __inca_test_listen__({node}, 'contextmenu', (e) => {{
                seen.push(e.type);
            }});
            "
        ))
        .unwrap();
    let mut cx = mount_clickable(cx, &host, &engine, node);
    let position = point(px(10.0), px(10.0));

    cx.simulate_mouse_down(position, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(position, MouseButton::Right, Modifiers::none());

    assert_eq!(engine.eval::<String>("seen.join()").unwrap(), "contextmenu");
}

#[gpui::test]
fn a_right_press_fires_the_pointer_mouse_and_contextmenu_fields(cx: &mut TestAppContext) {
    let (mut cx, engine, _parent, _child) = mount_recording_pair_with(
        cx,
        &["pointerdown", "mousedown", "contextmenu"],
        r"
        (e) => {
            if (e.target !== e.currentTarget) return;
            globalThis.seen.push([
                e.type, e.pointerId, e.pointerType, e.button, e.buttons, e.which, e.detail
            ].join());
        }
        ",
    );

    cx.simulate_mouse_down(
        point(px(10.0), px(10.0)),
        MouseButton::Right,
        Modifiers::none(),
    );

    let seen: String = engine.eval("JSON.stringify(globalThis.seen)").unwrap();
    assert_eq!(
        seen,
        r#"["pointerdown,1,mouse,2,2,3,0","mousedown,,,2,2,3,1","contextmenu,1,mouse,2,2,3,0"]"#
    );
}

/// Renders `node` with the scrollbars of a classic platform.
struct BarRoot {
    host: Rc<RefCell<Host>>,
    node: NodeId,
    dispatcher: EventDispatcher,
}

impl Render for BarRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.dispatcher.begin_frame(window, cx);
        let host = self.host.borrow();
        render_tree_with_events(&host.tree, self.node, &self.dispatcher, Scrollbars::Classic)
            .unwrap()
    }
}

/// A 100x100 container that scrolls on both axes and holds a 50x20 `input`, focused at the
/// start, and a tall child. Its listeners record `[type, target, currentTarget]` of
/// `pointerdown`, `mousedown`, `pointerup`, `mouseup` and `click`. A press at (92, 50) lies on its
/// vertical track.
fn mount_bar_container(cx: &mut TestAppContext) -> (VisualTestContext, Rc<Engine>, NodeId, NodeId) {
    let host = Rc::new(RefCell::new(Host::default()));
    let engine = test_support::engine_for(&host);
    let ids = engine
        .eval::<Vec<NodeId>>(
            r"
            const n = __inca_native__;
            const container = n.rootNodeId();
            n.setStyle(container, 'width', 100);
            n.setStyle(container, 'height', 100);
            n.setStyle(container, 'overflow', 'scroll');
            const input = n.createNode('input');
            n.setStyle(input, 'width', 50);
            n.setStyle(input, 'height', 20);
            n.appendChild(container, input);
            const tall = n.createNode('div');
            n.setStyle(tall, 'width', 300);
            n.setStyle(tall, 'height', 300);
            n.appendChild(container, tall);
            [container, input];
            ",
        )
        .unwrap();
    let (container, input) = (ids[0], ids[1]);
    engine.eval::<()>("globalThis.seen = [];").unwrap();
    for event in ["pointerdown", "mousedown", "pointerup", "mouseup", "click"] {
        engine
            .eval::<()>(&format!(
                r"
                __inca_test_listen__({container}, '{event}', (e) => {{
                    globalThis.seen.push([e.type, e.target, e.currentTarget]);
                }});
                "
            ))
            .unwrap();
    }
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));
    let window = cx.add_window(|_, _| BarRoot {
        host,
        node: container,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear(cx);
    })
    .unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    // A click on the input focuses it.
    cx.simulate_click(point(px(10.0), px(10.0)), Modifiers::none());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    engine.eval::<()>("globalThis.seen.length = 0;").unwrap();
    (cx, engine, container, input)
}

fn focused(engine: &Engine, node: NodeId) -> bool {
    engine
        .eval::<bool>(&format!("__inca_native__.focusedNode() === {node}"))
        .unwrap()
}

/// Presses and releases the left button on the vertical track of the container of
/// [`mount_bar_container`].
fn press_the_track(cx: &mut VisualTestContext) {
    let none = Modifiers::none();
    let track = point(px(92.0), px(50.0));
    cx.simulate_mouse_down(track, MouseButton::Left, none);
    cx.simulate_mouse_up(track, MouseButton::Left, none);
}

#[gpui::test]
fn a_press_on_a_scrollbar_keeps_the_focus(cx: &mut TestAppContext) {
    let (mut cx, engine, _container, input) = mount_bar_container(cx);
    assert!(focused(&engine, input));
    press_the_track(&mut cx);
    assert!(focused(&engine, input));
}

#[gpui::test]
fn a_scrollbar_press_fires_four_events_at_the_container(cx: &mut TestAppContext) {
    let (mut cx, engine, container, _input) = mount_bar_container(cx);
    press_the_track(&mut cx);
    let events: Vec<(String, NodeId, NodeId)> = serde_json::from_str(
        &engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
    )
    .unwrap();
    let want = ["pointerdown", "mousedown", "pointerup", "mouseup"]
        .map(|name| (name.to_owned(), container, container));
    assert_eq!(events, want);
}

#[gpui::test]
fn a_press_on_the_content_beside_a_scrollbar_blurs_the_focused_input(cx: &mut TestAppContext) {
    let (mut cx, engine, _container, input) = mount_bar_container(cx);
    assert!(focused(&engine, input));
    let none = Modifiers::none();

    cx.simulate_mouse_down(point(px(50.0), px(50.0)), MouseButton::Left, none);

    assert!(!focused(&engine, input));
}
