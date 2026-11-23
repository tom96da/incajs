// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The install of the members of `Element`, and `childElementCount`, which a freed node reads as
//! `0`.

use std::rc::Rc;

use rquickjs::{Ctx, Result as JsResult};

use inca_jsenv::required_prototype;

use crate::bindings::Host;
use crate::bindings::members::getter;
use crate::node::handle::PARENT_NODE;

/// Defines the members of `Element` and `childElementCount` of `Element`, `Document` and
/// `DocumentFragment`.
pub(crate) fn install(ctx: &Ctx<'_>, host: &Rc<Host>) -> JsResult<()> {
    let proto = required_prototype(ctx, "Element")?;
    super::names::install_names(ctx, &proto, host)?;
    super::selector::install_matches(ctx, &proto, host)?;
    super::capture::pointer_capture(ctx, &proto, host)?;
    for name in ["Element", "Document", "DocumentFragment"] {
        let proto = required_prototype(ctx, name)?;
        getter(
            ctx,
            &proto,
            host,
            "childElementCount",
            &PARENT_NODE,
            |_, host, id, _| {
                Ok(
                    u32::try_from(host.document.borrow().tree.child_count(id, true))
                        .unwrap_or(u32::MAX),
                )
            },
        )?;
    }
    Ok(())
}

/// Defines the attribute members and the markup getters of `Element`.
pub(crate) fn install_markup(ctx: &Ctx<'_>, host: &Rc<Host>) -> JsResult<()> {
    super::attributes::install_members(ctx, host)?;
    super::html::install_getters(ctx, host)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use serde_json::json;

    use crate::test_support::member_json;

    #[test]
    fn child_element_count_skips_text() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            n.appendChild(e.__node_id, n.createNode('span'));
            n.appendChild(e.__node_id, n.createNode('#text'));
            const doc = h(n.documentNodeId());
            return [e.childElementCount, doc.childElementCount];
            ",
        );

        assert_eq!(got, json!([1, 1]));
    }

    #[test]
    fn child_element_count_of_a_freed_node_is_zero() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            n.appendChild(e.__node_id, n.createNode('span'));
            free(e);
            return e.childElementCount;
            ",
        );

        assert_eq!(got, json!(0));
    }
}
