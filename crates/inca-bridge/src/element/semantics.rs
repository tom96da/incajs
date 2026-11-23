// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The meaning of an element: the members that read and write `tabIndex`, `role` and the ARIA
//! attributes that reflect text, and the natives that read and write the element references of the
//! ARIA attributes. A member of a node the host has freed reads -1 for `tabIndex` and `null` for the
//! text members, and ignores a write. A native throws a `TypeError` for an attribute outside the
//! element rows of the table, and for a freed node reads `null` and ignores a write.

use super::attributes::mark_focus_dirty_for;
use crate::bindings::convert::{JsNodeId, or_null};
use crate::bindings::members::accessor;
use crate::bindings::{Host, Natives};
use crate::node::handle::{ELEMENT, HTML_OR_SVG};
use inca_document::{ARIA_REFLECTIONS, AriaKind, TabIndex};
use inca_document::{AriaReferences, element_aria_attribute};
use inca_jsenv::required_prototype;
use inca_node::AttributeValue;
use rquickjs::convert::Coerced;
use rquickjs::{Ctx, Exception, FromJs, Result as JsResult, Value};
use std::rc::Rc;

/// Defines the text members of the ARIA table on the prototype of `Element`, and `tabIndex` on the
/// prototypes of `HTMLElement` and the foreign element.
pub(crate) fn install(ctx: &Ctx<'_>, host: &Rc<Host>) -> JsResult<()> {
    let element = required_prototype(ctx, "Element")?;
    for row in ARIA_REFLECTIONS
        .iter()
        .filter(|row| row.kind == AriaKind::Text)
    {
        let attribute = row.attribute;
        accessor(
            ctx,
            &element,
            host,
            row.idl,
            &ELEMENT,
            move |ctx, host, id, _| {
                or_null(
                    ctx,
                    host.document
                        .borrow()
                        .tree
                        .get(id)
                        .and_then(|node| node.attributes().get(attribute))
                        .map(AttributeValue::text),
                )
            },
            move |ctx, host, id, _, value| {
                let text = if value.is_null() || value.is_undefined() {
                    None
                } else {
                    Some(Coerced::<String>::from_js(ctx, value)?.0)
                };
                let mut host = host.document.borrow_mut();
                if host.tree.get(id).is_some() {
                    let _ = match text {
                        Some(text) => host.tree.set_attribute(id, attribute, text),
                        None => host.tree.remove_attribute(id, attribute),
                    };
                }
                Ok(())
            },
        )?;
    }
    for name in ["HTMLElement", "ForeignElement"] {
        accessor(
            ctx,
            &required_prototype(ctx, name)?,
            host,
            "tabIndex",
            &HTML_OR_SVG,
            |_, host, id, _| Ok(host.document.borrow().tree.tab_index(id).unwrap_or(-1)),
            |ctx, host, id, _, value| {
                let index = Coerced::<i32>::from_js(ctx, value)?.0;
                let mut host = host.document.borrow_mut();
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

/// `ariaElements` and `setAriaElements`: the natives behind the element members of the ARIA
/// table.
pub(crate) fn install_aria(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("ariaElements", aria_elements)?;
    n.set("setAriaElements", set_aria_elements)
}

/// Throws a `TypeError` unless `attribute` is a row of kind `Element` or `Elements`. Returns the
/// name of the attribute of the row.
fn element_row(ctx: &Ctx<'_>, attribute: &str) -> JsResult<&'static str> {
    element_aria_attribute(attribute).ok_or_else(|| {
        Exception::throw_type(
            ctx,
            &format!("{attribute} is not an attribute that refers to elements"),
        )
    })
}

/// The ids of the elements the attribute `attribute` of `node_id` refers to, `null` when it
/// refers to none.
fn aria_elements<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id), attribute| {
        let attribute = element_row(&ctx, &attribute)?;
        or_null(
            &ctx,
            host.document
                .borrow()
                .tree
                .aria_elements(node_id, attribute),
        )
    }
}

/// Keeps `ids` as the elements the attribute refers to, or removes the attribute for `null`.
fn set_aria_elements<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, String, Option<Vec<JsNodeId>>) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), attribute, ids| {
        let attribute = element_row(&ctx, &attribute)?;
        let ids = ids.map(|ids| ids.into_iter().map(|JsNodeId(id)| id).collect());
        let mut host = host.document.borrow_mut();
        if host.tree.get(node_id).is_some() {
            let _ = host.tree.set_aria_elements(node_id, attribute, ids);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::test_support::{engine_with_bindings, member_error, member_json};
    use inca_document::{ARIA_REFLECTIONS, AriaKind};
    use rstest::rstest;
    use serde_json::json;

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
        assert!(!host.document.borrow().focus.has_queued_work());
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

    #[rstest]
    #[case::absent(
        r"
        return mk('div').ariaLabel;
        ",
        json!(null)
    )]
    #[case::set(
        r"
        const e = mk('div');
        e.ariaLabel = 'x';
        return [e.ariaLabel, e.getAttribute('aria-label')];
        ",
        json!(["x", "x"])
    )]
    #[case::role(
        r"
        const e = mk('div');
        e.role = 'tab';
        return [e.role, e.getAttribute('role')];
        ",
        json!(["tab", "tab"])
    )]
    #[case::null_removes(
        r"
        const e = mk('div');
        e.ariaLabel = 'x';
        e.ariaLabel = null;
        return e.hasAttribute('aria-label');
        ",
        json!(false)
    )]
    #[case::undefined_removes(
        r"
        const e = mk('div');
        e.ariaLabel = 'x';
        e.ariaLabel = undefined;
        return e.hasAttribute('aria-label');
        ",
        json!(false)
    )]
    #[case::number_becomes_text(
        r"
        const e = mk('div');
        e.ariaLevel = 5;
        return e.getAttribute('aria-level');
        ",
        json!("5")
    )]
    #[case::empty_text_sets_an_empty_attribute(
        r"
        const e = mk('div');
        e.ariaLabel = '';
        return [e.hasAttribute('aria-label'), e.ariaLabel];
        ",
        json!([true, ""])
    )]
    fn a_text_member_reflects_its_attribute(#[case] code: &str, #[case] want: serde_json::Value) {
        let (got, _host) = member_json(code);

        assert_eq!(got, want);
    }

    #[test]
    fn a_symbol_write_to_a_text_member_is_rejected() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            e.ariaLabel = 'x';
            let name = 'none';
            try {
                e.ariaLabel = Symbol('y');
            } catch (err) {
                name = err.name;
            }
            return [name, e.ariaLabel];
            ",
        );

        assert_eq!(got, json!(["TypeError", "x"]));
    }

    #[test]
    fn a_freed_element_reads_null_from_a_text_member() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            free(e);
            return [e.ariaLabel, e.role];
            ",
        );

        assert_eq!(got, json!([null, null]));
    }

    #[test]
    fn a_write_to_a_text_member_of_a_freed_element_throws_nothing() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            free(e);
            e.ariaLabel = 'x';
            return 'done';
            ",
        );

        assert_eq!(got, json!("done"));
    }

    #[rstest]
    #[case::no_namespace("null", "x")]
    #[case::svg("'http://www.w3.org/2000/svg'", "circle")]
    fn a_text_member_reads_and_writes_on_an_element_outside_html(
        #[case] namespace: &str,
        #[case] tag: &str,
    ) {
        let (got, _host) = member_json(&format!(
            r"
            const e = h(n.createElementNS({namespace}, '{tag}')[0]);
            e.ariaLabel = 'p';
            return e.ariaLabel;
            "
        ));

        assert_eq!(got, json!("p"));
    }

    #[test]
    fn the_text_members_are_enumerable_configurable_accessors_of_element() {
        let (engine, _host) = engine_with_bindings();
        let names: Vec<&str> = ARIA_REFLECTIONS
            .iter()
            .filter(|row| row.kind == AriaKind::Text)
            .map(|row| row.idl)
            .collect();

        let got: Vec<String> = serde_json::from_value(crate::test_support::json_of(
            &engine,
            &format!(
                r"
                (() => {{
                    const p = __inca_native__.interfacePrototypes();
                    return {names:?}.filter((k) => {{
                        const d = Object.getOwnPropertyDescriptor(p.Element, k);
                        const other = [p.HTMLElement, p.ForeignElement].some((q) => k in Object.getOwnPropertyDescriptors(q));
                        return !(d && d.get && d.set && d.enumerable && d.configurable) || other;
                    }});
                }})()
                "
            ),
        ))
        .unwrap();

        assert_eq!(names.len(), 44);
        assert_eq!(got, Vec::<String>::new());
    }

    #[test]
    fn a_freed_node_reads_null() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            const id = e.__node_id;
            free(e);
            return n.ariaElements(id, 'aria-owns');
            ",
        );

        assert_eq!(got, json!(null));
    }

    #[test]
    fn a_set_reads_back_the_ids_in_order() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            const a = mk('span');
            const b = mk('span');
            n.setAriaElements(e.__node_id, 'aria-owns', [b.__node_id, a.__node_id]);
            return [n.ariaElements(e.__node_id, 'aria-owns'), e.getAttribute('aria-owns'), b.__node_id, a.__node_id];
            ",
        );

        assert_eq!(got[0], json!([got[2], got[3]]));
        assert_eq!(got[1], json!(""));
    }

    #[test]
    fn a_null_set_removes_the_attribute() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            n.setAriaElements(e.__node_id, 'aria-owns', [e.__node_id]);
            n.setAriaElements(e.__node_id, 'aria-owns', null);
            return [e.hasAttribute('aria-owns'), n.ariaElements(e.__node_id, 'aria-owns')];
            ",
        );

        assert_eq!(got, json!([false, null]));
    }

    #[test]
    fn a_malformed_id_is_rejected() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            let name = 'none';
            try {
                n.setAriaElements(e.__node_id, 'aria-owns', [e.__node_id, -1]);
            } catch (err) {
                name = err.name;
            }
            return [name, e.hasAttribute('aria-owns')];
            ",
        );

        assert_eq!(got, json!(["TypeError", false]));
    }

    #[rstest]
    #[case::read_a_text_attribute("n.ariaElements(e.__node_id, 'aria-label')")]
    #[case::read_an_unknown_attribute("n.ariaElements(e.__node_id, 'title')")]
    #[case::write_a_text_attribute("n.setAriaElements(e.__node_id, 'aria-label', [])")]
    fn an_attribute_that_refers_to_no_element_throws(#[case] call: &str) {
        let (got, _host) = member_json(&format!(
            r"
            const e = mk('div');
            e.setAttribute('aria-label', 'x');
            let name = 'none';
            try {{
                {call};
            }} catch (err) {{
                name = err.name;
            }}
            return [name, e.getAttribute('aria-label')];
            "
        ));

        assert_eq!(got, json!(["TypeError", "x"]));
    }
}
