// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[test]
fn remove_style_unsets_the_key_and_rejects_an_unknown_node() {
    let (engine, host) = engine_with_bindings();

    let ids: Vec<u32> = engine
        .eval(
            r"
            const n = __inca_native__;
            const node = n.createNode('div');
            n.setStyle(node, 'gap', 8);
            n.setStyle(node, 'width', 1);
            n.removeStyle(node, 'gap');
            let caught = false;
            try { n.removeStyle(999, 'gap'); } catch (e) { caught = true; }
            [node, caught ? 1 : 0];
            ",
        )
        .unwrap();

    let [node, caught] = ids[..] else {
        panic!("expected the node id and the caught flag");
    };
    assert_eq!(caught, 1);
    let host = host.borrow();
    let styles = host.tree.get(node).unwrap().style_props();
    assert!(styles.get("gap").is_none());
    assert!(styles.get("width").is_some());
}

/// Evaluates `script` against bindings whose style warnings are
/// collected, and returns them with the host.
fn warnings_from(script: &str) -> (Rc<RefCell<Vec<String>>>, Rc<RefCell<Host>>) {
    let (engine, host) = engine_with_bindings();
    let warnings = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&warnings);
    host.borrow_mut().warn = Rc::new(move |line| sink.borrow_mut().push(line.to_owned()));
    engine.eval::<()>(script).unwrap();
    (warnings, host)
}

#[test]
fn set_style_warns_once_per_unknown_key() {
    let (warnings, _) = warnings_from(
        "const n = __inca_native__.createNode('div'); \
         __inca_native__.setStyle(n, 'flexDirections', 'column');",
    );

    assert_eq!(
        *warnings.borrow(),
        ["node 1 (div): ignoring unknown style key `flexDirections`"]
    );
}

#[test]
fn set_style_warns_for_each_set_of_a_bad_value() {
    let (warnings, _) = warnings_from(
        "const n = __inca_native__.createNode('div'); \
         __inca_native__.setStyle(n, 'background', -1); \
         __inca_native__.setStyle(n, 'background', -1); \
         __inca_native__.setStyle(n, 'background', 0xff0000);",
    );

    assert_eq!(
        *warnings.borrow(),
        ["node 1 (div): ignoring invalid color for style key `background`"; 2]
    );
}

#[test]
fn a_failed_set_style_emits_no_warning() {
    let (warnings, _) = warnings_from(
        "try { __inca_native__.setStyle(999, 'bogus', 1); } catch (e) {} \
         const n = __inca_native__.createNode('div'); \
         try { __inca_native__.setStyle(n, 'bogus', { nested: true }); } catch (e) {}",
    );

    assert!(warnings.borrow().is_empty(), "{:?}", warnings.borrow());
}

#[test]
fn set_style_names_the_tag_of_the_node() {
    let (warnings, _) = warnings_from(
        "const n = __inca_native__.createNode('text'); \
         __inca_native__.setStyle(n, 'bogus', 1);",
    );

    assert_eq!(
        *warnings.borrow(),
        ["node 1 (text): ignoring unknown style key `bogus`"]
    );
}

#[test]
fn set_style_is_quiet_for_known_keys_and_box_keys() {
    let (warnings, _) = warnings_from(
        "const n = __inca_native__.createNode('div'); \
         for (const k of ['display', 'padding', 'padding_x', 'margin_left', 'gap']) \
           __inca_native__.setStyle(n, k, 'flex'); \
         __inca_native__.setStyle(n, 'text_color', '#fff');",
    );

    assert!(warnings.borrow().is_empty(), "{:?}", warnings.borrow());
}

#[test]
fn a_stored_bad_style_does_not_warn_again_when_the_tree_is_read() {
    let (warnings, host) = warnings_from(
        "const n = __inca_native__.createNode('div'); \
         __inca_native__.setStyle(n, 'bogus', 1);",
    );
    assert_eq!(warnings.borrow().len(), 1);

    let host = host.borrow();
    for _ in 0..3 {
        inca_gpui::build_spec(&host.tree, 1).unwrap();
    }
    assert_eq!(warnings.borrow().len(), 1);
}

#[test]
fn set_style_unknown_node_id_raises_catchable_exception() {
    let (engine, _host) = engine_with_bindings();

    let caught: bool = engine
        .eval(
            r"
            let caught = false;
            try {
                __inca_native__.setStyle(999, 'display', 'flex');
            } catch (e) {
                caught = true;
            }
            caught;
            ",
        )
        .unwrap();

    assert!(
        caught,
        "an unknown node id must raise a catchable exception"
    );
}

#[test]
fn set_style_non_primitive_value_raises_catchable_exception() {
    let (engine, _host) = engine_with_bindings();

    let caught: bool = engine
        .eval(
            r"
            const node = __inca_native__.createNode('div');
            let caught = false;
            try {
                __inca_native__.setStyle(node, 'bad', { nested: true });
            } catch (e) {
                caught = true;
            }
            caught;
            ",
        )
        .unwrap();

    assert!(
        caught,
        "a non-primitive style value must raise a catchable exception"
    );
}
