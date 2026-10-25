// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use rstest::rstest;

/// Captures every line written to it, so a test can read back what was sent.
#[derive(Default)]
struct CapturingWriter(RefCell<Vec<String>>);

impl Writer for CapturingWriter {
    fn write_line(&self, line: String) {
        self.0.borrow_mut().push(line);
    }
}

#[test]
fn inca_dev_exists_only_with_a_writer() {
    let session = load("globalThis.hasDev = typeof __inca_dev__ !== 'undefined';").unwrap();

    assert!(!session.engine.eval::<bool>("globalThis.hasDev;").unwrap());
}

#[test]
fn inca_dev_send_writes_a_notification_to_the_writer() {
    let capturing = Rc::new(CapturingWriter::default());
    let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;

    Session::begin(
        TEST_ENTRY_PATH,
        r"__inca_dev__.send('testEvent', JSON.stringify({ a: 1 }));",
        stderr_reporter(),
        Some(&writer),
    )
    .unwrap();

    let sent = capturing.0.borrow();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].contains(r#""method":"testEvent""#));
    assert!(sent[0].contains(r#""a":1"#));
}

fn capturing_writer() -> SharedWriter {
    Rc::new(CapturingWriter::default())
}

fn begin(source: &str) -> Result<Loading, EngineError> {
    Session::begin(
        TEST_ENTRY_PATH,
        source,
        stderr_reporter(),
        Some(&capturing_writer()),
    )
}

#[test]
fn a_pending_entry_begins_as_a_loading_that_settles_when_resolved() {
    let loading =
        begin("await new Promise((r) => { globalThis.release = r; }); globalThis.done = true;")
            .unwrap();

    assert_eq!(loading.poll(), None);
    loading.session().engine.eval::<()>("release();").unwrap();
    assert_eq!(loading.poll(), Some(Ok(())));
    let session = loading.into_session();
    assert!(session.engine.eval::<bool>("globalThis.done;").unwrap());
}

#[test]
fn a_load_failed_report_in_the_synchronous_part_shows_right_after_begin() {
    let loading = begin("__inca_dev__.setLoadFailed(true); await new Promise(() => {});").unwrap();

    assert!(loading.start_failed());
    loading
        .session()
        .engine
        .eval::<()>("__inca_dev__.setLoadFailed(false);")
        .unwrap();
    assert!(!loading.start_failed());
}

#[test]
fn reported_failure_is_true_once_per_report_even_when_cleared() {
    let loading = begin("await new Promise(() => {});").unwrap();
    let eval = |script| loading.session().engine.eval::<()>(script).unwrap();

    assert!(!loading.reported_failure());
    eval("__inca_dev__.setLoadFailed(true); __inca_dev__.setLoadFailed(false);");
    assert!(loading.reported_failure());
    assert!(!loading.reported_failure());
    eval("__inca_dev__.setLoadFailed(true);");
    assert!(loading.reported_failure());
}

#[rstest]
#[case::throws_synchronously("throw new Error('sync');")]
#[case::throws_after_an_await("await Promise.resolve(); throw new Error('async');")]
fn begin_returns_the_error_of_an_entry_that_throws_or_rejects_before_it_waits(
    #[case] source: &str,
) {
    let err = begin(source).err().unwrap();
    assert!(err.message().starts_with("Error: "), "{source}");
    assert!(err.stack().is_some(), "{source}");
}

#[test]
fn load_reports_a_pending_entry_as_an_error() {
    let err = load("await new Promise(() => {});").err().unwrap();

    assert!(!err.message().is_empty());
}

#[test]
fn a_bundle_can_log_while_it_evaluates() {
    assert!(
        load("console.log('mounting', { ready: true });").is_ok(),
        "console has to exist before the bundle runs, not after"
    );
}

#[test]
fn a_bundle_that_throws_yields_nothing() {
    let err = load("throw new Error('boom');").err().unwrap();

    assert_eq!(err.message(), "Error: boom");
    assert!(
        err.stack().is_some(),
        "which the client reports as data.stack"
    );
}

#[test]
fn a_thrown_error_names_the_bundle_from_the_app_root() {
    let dir = crate::tests::ScratchDir::new("root-relative");
    std::fs::create_dir_all(dir.join("app/dist")).unwrap();
    std::fs::write(dir.join("app/dist/inca.json"), r#"{"root":".."}"#).unwrap();
    let entry = dir.join("app/dist/bundle.js");
    let entry_path = entry.to_str().unwrap();

    let err = Session::load(entry_path, "throw new Error('boom');", stderr_reporter())
        .err()
        .unwrap();

    let stack = err.stack().unwrap();
    assert!(stack.contains("(dist/bundle.js:"), "{stack}");
}

struct ClosingWriter(Rc<Cell<bool>>);

impl Writer for ClosingWriter {
    fn write_line(&self, _line: String) {}

    fn close(&self) {
        self.0.set(true);
    }
}

#[gpui::test]
fn quitting_closes_the_writer(cx: &mut TestAppContext) {
    let closed = Rc::new(Cell::new(false));
    cx.update(|cx| {
        close_writer_on_quit(cx, Rc::new(ClosingWriter(Rc::clone(&closed))));
        cx.shutdown();
    });
    cx.run_until_parked();

    assert!(closed.get());
}

// Attaches one node under the root and registers a listener on it, so a
// reload has both tree and registry state that could leak.
const MOUNTING_BUNDLE: &str = r"
    const node = __inca_native__.createNode('div');
    __inca_native__.appendChild(__inca_native__.rootNodeId(), node);
    __inca_native__.addEventListener(node, 'click');
";

#[test]
fn a_reload_leaves_no_stale_nodes_listeners_or_callbacks() {
    let first = load(MOUNTING_BUNDLE).unwrap();
    first
        .engine
        .eval::<()>("__inca_callbacks__[0] = () => {};")
        .unwrap();
    let first_child = first
        .host
        .borrow()
        .tree
        .get(first.root())
        .unwrap()
        .children()[0];

    let second = load(MOUNTING_BUNDLE).unwrap();
    drop(first);

    let host = second.host.borrow();
    let children = host.tree.get(second.root()).unwrap().children();
    assert_eq!(children.len(), 1, "the reloaded tree must not accumulate");
    assert_eq!(
        children[0], first_child,
        "ids restart, so the reloaded tree replaces the first"
    );
    let ids: Vec<u32> = host
        .listeners
        .listeners_for(inca_bridge::EventTargetId::Node(children[0]), "click")
        .iter()
        .map(|l| l.callback)
        .collect();
    assert_eq!(ids, &[0]);
    assert_eq!(
        second
            .engine
            .eval::<i32>("Object.keys(globalThis.__inca_callbacks__).length;")
            .unwrap(),
        0,
        "the fresh engine must not carry the previous callback registry"
    );
}

const THROWING_LISTENER_BUNDLE: &str = r"
    const node = __inca_native__.createNode('div');
    __inca_native__.appendChild(__inca_native__.rootNodeId(), node);
    __inca_native__.addEventListener(node, 'click');
    globalThis.__inca_callbacks__[0] = () => {
        throw new Error('boom');
    };
";

#[gpui::test]
fn a_throwing_listener_is_reported_exactly_once_to_the_writer(cx: &mut TestAppContext) {
    let capturing = Rc::new(CapturingWriter::default());
    let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;
    let reporter = reporter_for(&writer);

    let window = cx.update(|cx| {
        let session = Session::begin(
            TEST_ENTRY_PATH,
            THROWING_LISTENER_BUNDLE,
            reporter,
            Some(&writer),
        )
        .unwrap()
        .into_session();
        open(cx, session, TEST_ENTRY_PATH).unwrap()
    });
    cx.run_until_parked();

    cx.update(|cx| {
        window
            .update(cx, |app: &mut HostedApp, window, cx| {
                let node = app
                    .session
                    .host
                    .borrow()
                    .tree
                    .get(app.session.root())
                    .unwrap()
                    .children()[0];
                app.session
                    .dispatcher
                    .fire(node, "click", &EventPayload::None, window, cx);
            })
            .unwrap();
    });
    cx.run_until_parked();

    let sent = capturing.0.borrow();
    assert_eq!(
        sent.len(),
        1,
        "a throwing listener must be reported exactly once, not once per drain"
    );
    assert!(sent[0].contains(r#""method":"appError""#));
    assert!(sent[0].contains("boom"));
}

#[test]
fn a_listener_error_is_reported_when_the_entry_throws_after_dispatching_it() {
    let capturing = Rc::new(CapturingWriter::default());
    let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;

    let err = Session::begin(
        TEST_ENTRY_PATH,
        &format!(
            "{THROWING_LISTENER_BUNDLE}\n__inca_native__.click(node);\nthrow new Error('entry');"
        ),
        reporter_for(&writer),
        Some(&writer),
    )
    .err()
    .unwrap();

    assert_eq!(err.message(), "Error: entry");
    let sent = capturing.0.borrow();
    assert_eq!(
        sent.len(),
        1,
        "the listener error is reported before the session is dropped"
    );
    assert!(sent[0].contains(r#""method":"appError""#));
    assert!(sent[0].contains("boom"));
}

#[test]
fn a_listener_error_is_reported_when_load_throws_after_dispatching_it() {
    let capturing = Rc::new(CapturingWriter::default());
    let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;

    let err = Session::load(
        TEST_ENTRY_PATH,
        &format!(
            "{THROWING_LISTENER_BUNDLE}\n__inca_native__.click(node);\nthrow new Error('entry');"
        ),
        reporter_for(&writer),
    )
    .err()
    .unwrap();

    assert_eq!(err.message(), "Error: entry");
    let sent = capturing.0.borrow();
    assert_eq!(
        sent.len(),
        1,
        "the listener error is reported before the session is dropped"
    );
    assert!(sent[0].contains(r#""method":"appError""#));
    assert!(sent[0].contains("boom"));
}

#[test]
fn a_job_error_is_reported_once_when_begin_rejects_after_it() {
    let capturing = Rc::new(CapturingWriter::default());
    let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;

    let err = Session::begin(
        TEST_ENTRY_PATH,
        r"
        queueMicrotask(() => { throw new Error('side'); });
        await Promise.resolve(); throw new Error('entry');
        ",
        reporter_for(&writer),
        Some(&writer),
    )
    .err()
    .unwrap();

    assert_eq!(err.message(), "Error: entry");
    let sent = capturing.0.borrow();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].contains("side"));
}

#[test]
fn a_job_error_is_reported_once_when_load_rejects_after_it() {
    let capturing = Rc::new(CapturingWriter::default());
    let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;

    let err = Session::load(
        TEST_ENTRY_PATH,
        r"
        queueMicrotask(() => { throw new Error('side'); });
        await Promise.resolve(); throw new Error('entry');
        ",
        reporter_for(&writer),
    )
    .err()
    .unwrap();

    assert_eq!(err.message(), "Error: entry");
    let sent = capturing.0.borrow();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].contains("side"));
}

#[gpui::test]
fn a_listener_that_throws_during_load_is_reported_when_the_window_opens(cx: &mut TestAppContext) {
    let capturing = Rc::new(CapturingWriter::default());
    let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;
    let reporter = reporter_for(&writer);
    let source = format!("{THROWING_LISTENER_BUNDLE}\n__inca_native__.click(node);");

    cx.update(|cx| {
        let session = Session::load(TEST_ENTRY_PATH, &source, reporter).unwrap();
        assert!(
            capturing.0.borrow().is_empty(),
            "nothing is reported before open"
        );
        open(cx, session, TEST_ENTRY_PATH).unwrap();
    });
    cx.run_until_parked();

    let sent = capturing.0.borrow();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].contains(r#""method":"appError""#));
    assert!(sent[0].contains("boom"));
}
