// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The members of the elements that read what an element means: `tabIndex`. A node the host has
//! freed reads -1 and ignores a write.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::convert::Coerced;
use rquickjs::{Ctx, FromJs, Result as JsResult};

use inca_jsenv::required_prototype;
use inca_node::AttributeValue;

use super::accessor;
use crate::bindings::attributes::mark_focus_dirty_for;
use crate::bindings::handle::HTML_OR_SVG;
use crate::bindings::host::Host;

/// Defines `tabIndex` on the prototypes of `HTMLElement` and the foreign element.
pub(super) fn install(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>) -> JsResult<()> {
    for name in ["HTMLElement", "ForeignElement"] {
        accessor(
            ctx,
            &required_prototype(ctx, name)?,
            host,
            "tabIndex",
            &HTML_OR_SVG,
            |_, host, id, _| Ok(host.borrow().tree.tab_index(id).unwrap_or(-1)),
            |ctx, host, id, _, value| {
                let index = Coerced::<i32>::from_js(ctx, value)?.0;
                let mut host = host.borrow_mut();
                if host.tree.get(id).is_some() {
                    let _ = host.tree.set_attribute(
                        id,
                        "tabindex",
                        AttributeValue::Number(f64::from(index)),
                    );
                    mark_focus_dirty_for(&mut host, id, "tabindex");
                }
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;
    use serde_json::json;

    use crate::test_support::{engine_with_bindings, member_error, member_json};

    #[rstest]
    #[case::explicit(
        r"
        const e = mk('div');
        n.setAttribute(e.__node_id, 'tabindex', '5');
        return e.tabIndex;
        ",
        5
    )]
    #[case::explicit_minus_one_on_a_button(
        r"
        const e = mk('button');
        n.setAttribute(e.__node_id, 'tabindex', '-1');
        return e.tabIndex;
        ",
        -1
    )]
    #[case::default_of_a_button("return mk('button').tabIndex;", 0)]
    #[case::default_of_a_div("return mk('div').tabIndex;", -1)]
    fn tab_index_reads_the_tree(#[case] code: &str, #[case] want: i32) {
        let (got, _host) = member_json(code);

        assert_eq!(got, json!(want));
    }

    #[rstest]
    #[case::wrapped("2 ** 32 + 1", 1)]
    #[case::fraction("-3.9", -3)]
    #[case::text("'abc'", 0)]
    #[case::null("null", 0)]
    #[case::array("[7]", 7)]
    fn a_tab_index_write_stores_the_int32_value(#[case] value: &str, #[case] want: i32) {
        let (got, _host) = member_json(&format!(
            r"
            const e = mk('div');
            e.tabIndex = {value};
            return [e.getAttribute('tabindex'), e.tabIndex];
            "
        ));

        assert_eq!(got, json!([want.to_string(), want]));
    }

    #[test]
    fn a_symbol_write_throws_and_stores_nothing() {
        let (engine, _host) = engine_with_bindings();

        let got: String = engine
            .eval(
                r"
                (() => {
                    const n = __inca_native__;
                    const id = n.createNode('div');
                    n.appendChild(n.rootNodeId(), id);
                    const e = n.nodeHandle(id)[0];
                    let name = 'none';
                    try {
                        e.tabIndex = Symbol('x');
                    } catch (err) {
                        name = err.name;
                    }
                    return name + ':' + e.hasAttribute('tabindex');
                })()
                ",
            )
            .unwrap();

        assert_eq!(got, "TypeError:false");
    }

    #[test]
    fn a_tab_index_write_makes_the_element_focusable() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            e.tabIndex = 0;
            e.focus();
            return e.matches(':focus');
            ",
        );

        assert_eq!(got, json!(true));
    }

    #[test]
    fn a_freed_element_reads_minus_one_and_ignores_a_write() {
        let (got, host) = member_json(
            r"
            const e = mk('button');
            free(e);
            e.tabIndex = 3;
            return e.tabIndex;
            ",
        );

        assert_eq!(got, json!(-1));
        assert!(!host.borrow().focus.has_queued_work());
    }

    #[test]
    fn a_receiver_that_is_no_element_throws() {
        assert_eq!(
            member_error(
                r"
                const proto = __inca_native__.interfacePrototypes().HTMLElement;
                const d = Object.getOwnPropertyDescriptor(proto, 'tabIndex');
                d.get.call({});
                "
            ),
            "TypeError"
        );
    }
}
