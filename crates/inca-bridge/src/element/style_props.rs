// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The style props of an element: the natives `setStyle` and `removeStyle`, which write and remove
//! a style prop and print a warning once for a key or value the host does not support.

use std::rc::Rc;

use rquickjs::{Ctx, Result as JsResult, Value};

use super::attributes::attribute_value_from_js;
use crate::bindings::Host;
use crate::bindings::convert::{JsNodeId, throw_tree_error};

pub(super) fn set_style<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, String, Value<'js>) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), key, value| {
        let value = attribute_value_from_js(&ctx, &value)?;
        let mut host = host.document.borrow_mut();
        let host = &mut *host;
        host.set_style_prop(node_id, &key, value)
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

pub(super) fn remove_style<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), key| {
        let mut host = host.document.borrow_mut();
        let host = &mut *host;
        host.remove_style_prop(node_id, &key)
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::Host;
    use crate::test_support::engine_with_bindings;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn remove_style_on_an_unknown_node_throws() {
        let (engine, _host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                let caught = false;
                try {
                    n.removeStyle(999, 'gap');
                } catch (e) {
                    caught = true;
                }
                caught;
                ",
            )
            .unwrap();

        assert!(caught);
    }

    /// Evaluates `script` against bindings whose style warnings are
    /// collected, and returns them with the host.
    fn warnings_from(script: &str) -> (Rc<RefCell<Vec<String>>>, Rc<Host>) {
        let (engine, host) = engine_with_bindings();
        let warnings = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&warnings);
        host.document
            .borrow_mut()
            .warnings
            .set_sink(Rc::new(move |line| sink.borrow_mut().push(line.to_owned())));
        engine.eval::<()>(script).unwrap();
        (warnings, host)
    }

    #[test]
    fn set_style_warns_once_per_unknown_key() {
        let (warnings, _) = warnings_from(
            r"
            const n = __inca_native__.createNode('div');
            __inca_native__.setStyle(n, 'flexDirections', 'column');
            ",
        );

        assert_eq!(
            *warnings.borrow(),
            ["node 2 (div): ignoring unknown style key `flexDirections`"]
        );
    }

    #[test]
    fn a_bad_value_warns_once_for_a_node_and_key() {
        let (warnings, _) = warnings_from(
            r"
            const n = __inca_native__.createNode('div');
            __inca_native__.setStyle(n, 'background', -1);
            __inca_native__.setStyle(n, 'background', -1);
            __inca_native__.setStyle(n, 'background', 0xff0000);
            __inca_native__.setStyle(n, 'background', -1);
            ",
        );

        assert_eq!(
            *warnings.borrow(),
            ["node 2 (div): ignoring invalid color for style key `background`"]
        );
    }

    #[test]
    fn remove_style_does_not_make_a_key_warn_again() {
        let (warnings, _) = warnings_from(
            r"
            const n = __inca_native__.createNode('div');
            __inca_native__.setStyle(n, 'bogus', 1);
            __inca_native__.removeStyle(n, 'bogus');
            __inca_native__.setStyle(n, 'bogus', 1);
            ",
        );

        assert_eq!(warnings.borrow().len(), 1, "{:?}", warnings.borrow());
    }

    #[test]
    fn each_node_warns_for_the_same_bad_key() {
        let (warnings, _) = warnings_from(
            r"
            const a = __inca_native__.createNode('div');
            const b = __inca_native__.createNode('div');
            __inca_native__.setStyle(a, 'bogus', 1);
            __inca_native__.setStyle(b, 'bogus', 1);
            __inca_native__.setStyle(a, 'bogus', 1);
            ",
        );

        assert_eq!(
            *warnings.borrow(),
            [
                "node 2 (div): ignoring unknown style key `bogus`",
                "node 3 (div): ignoring unknown style key `bogus`",
            ]
        );
    }

    #[test]
    fn each_spelling_of_a_bad_key_warns() {
        let (warnings, _) = warnings_from(
            r"
            const n = __inca_native__.createNode('div');
            __inca_native__.setStyle(n, 'bogus-key', 1);
            __inca_native__.setStyle(n, 'bogusKey', 1);
            __inca_native__.setStyle(n, 'bogus-key', 1);
            __inca_native__.setStyle(n, 'bogusKey', 1);
            ",
        );

        assert_eq!(
            *warnings.borrow(),
            [
                "node 2 (div): ignoring unknown style key `bogus-key`",
                "node 2 (div): ignoring unknown style key `bogusKey`",
            ]
        );
    }

    #[test]
    fn each_bad_key_of_a_node_warns_once() {
        let (warnings, _) = warnings_from(
            r"
            const n = __inca_native__.createNode('div');
            __inca_native__.setStyle(n, 'bogus', 1);
            __inca_native__.setStyle(n, 'width', '50%');
            __inca_native__.setStyle(n, 'bogus', 1);
            __inca_native__.setStyle(n, 'width', '50%');
            ",
        );

        assert_eq!(
            *warnings.borrow(),
            [
                "node 2 (div): ignoring unknown style key `bogus`",
                "node 2 (div): ignoring unsupported value for style key `width`",
            ]
        );
    }

    #[test]
    fn a_destroyed_node_forgets_its_style_warnings() {
        let (_, host) = warnings_from(
            r"
            const n = __inca_native__.createNode('div');
            __inca_native__.setStyle(n, 'bogus', 1);
            __inca_native__.destroyNode(n);
            ",
        );

        assert!(host.document.borrow().warnings.is_empty());
    }

    #[test]
    fn a_failed_set_style_emits_no_warning() {
        let (warnings, host) = warnings_from(
            r"
            try {
                __inca_native__.setStyle(999, 'bogus', 1);
            } catch (e) {}
            const n = __inca_native__.createNode('div');
            try {
                __inca_native__.setStyle(n, 'bogus', { nested: true });
            } catch (e) {}
            ",
        );

        assert!(warnings.borrow().is_empty(), "{:?}", warnings.borrow());
        assert!(host.document.borrow().warnings.is_empty());
    }

    #[test]
    fn set_style_names_the_tag_of_the_node() {
        let (warnings, _) = warnings_from(
            r"
            const n = __inca_native__.createNode('#text');
            __inca_native__.setStyle(n, 'bogus', 1);
            ",
        );

        assert_eq!(
            *warnings.borrow(),
            ["node 2 (#text): ignoring unknown style key `bogus`"]
        );
    }

    #[test]
    fn set_style_is_quiet_for_known_keys_and_box_keys() {
        let (warnings, _) = warnings_from(
            r"
            const n = __inca_native__.createNode('div');
            __inca_native__.setStyle(n, 'display', 'flex');
            for (const k of ['padding', 'padding_x', 'margin_left', 'gap']) {
                __inca_native__.setStyle(n, k, 4);
            }
            __inca_native__.setStyle(n, 'text_color', '#fff');
            ",
        );

        assert!(warnings.borrow().is_empty(), "{:?}", warnings.borrow());
    }

    #[test]
    fn a_stored_bad_style_does_not_warn_again_when_the_tree_is_read() {
        let (warnings, host) = warnings_from(
            r"
            const n = __inca_native__.createNode('div');
            __inca_native__.setStyle(n, 'bogus', 1);
            ",
        );
        assert_eq!(warnings.borrow().len(), 1);

        let host = host.document.borrow();
        for _ in 0..3 {
            inca_style::build_spec(host.tree(), &host.style, 1).unwrap();
        }
        assert_eq!(warnings.borrow().len(), 1);
    }

    #[test]
    fn set_style_unknown_node_id_throws_catchable_exception() {
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
}
