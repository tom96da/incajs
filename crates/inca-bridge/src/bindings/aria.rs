// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Natives that read and write the element references of the ARIA attributes.
//!
//! An attribute outside the element rows of the table throws a `TypeError`. A freed node reads
//! `null` and ignores a write.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Exception, Result as JsResult, Value};

use inca_document::{AriaKind, AriaReferences, aria_reflection};

use super::Natives;
use super::convert::{JsNodeId, or_null};
use super::host::Host;

/// `ariaElements` and `setAriaElements`: the natives behind the element members of the ARIA
/// table.
pub(super) fn install_aria(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("ariaElements", aria_elements)?;
    n.set("setAriaElements", set_aria_elements)
}

/// Throws a `TypeError` unless `attribute` is a row of kind `Element` or `Elements`. Returns the
/// name of the attribute of the row.
fn element_row(ctx: &Ctx<'_>, attribute: &str) -> JsResult<&'static str> {
    match aria_reflection(attribute) {
        Some(row) if row.kind != AriaKind::Text => Ok(row.attribute),
        _ => Err(Exception::throw_type(
            ctx,
            &format!("{attribute} is not an attribute that refers to elements"),
        )),
    }
}

/// The ids of the elements the attribute `attribute` of `node_id` refers to, `null` when it
/// refers to none.
fn aria_elements<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id), attribute| {
        let attribute = element_row(&ctx, &attribute)?;
        or_null(&ctx, host.borrow().tree.aria_elements(node_id, attribute))
    }
}

/// Keeps `ids` as the elements the attribute refers to, or removes the attribute for `null`.
fn set_aria_elements<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String, Option<Vec<JsNodeId>>) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), attribute, ids| {
        let attribute = element_row(&ctx, &attribute)?;
        let ids = ids.map(|ids| ids.into_iter().map(|JsNodeId(id)| id).collect());
        let mut host = host.borrow_mut();
        if host.tree.get(node_id).is_some() {
            let _ = host.tree.set_aria_elements(node_id, attribute, ids);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;
    use serde_json::json;

    use crate::test_support::member_json;

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
