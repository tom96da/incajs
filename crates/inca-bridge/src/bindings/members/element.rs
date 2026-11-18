// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The name members of `Element`: `tagName`, `localName`, `namespaceURI` and `prefix`. An element
//! the host has freed reads `""` for the first two and `null` for the others.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Result as JsResult, Value};

use inca_jsenv::class::define_this_getter;
use inca_jsenv::required_prototype;
use inca_node::VirtualNode;

use crate::bindings::handle::{ELEMENT, receiver_of};
use crate::bindings::host::Host;

/// Defines the name getters on `Element.prototype`.
pub(super) fn install(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>) -> JsResult<()> {
    let proto = required_prototype(ctx, "Element")?;
    define_name(ctx, &proto, host, "tagName", |node| {
        node.map_or_else(String::new, |node| node.dom_tag_name().into_owned())
    })?;
    define_name(ctx, &proto, host, "localName", |node| {
        node.map_or_else(String::new, |node| node.tag_name().to_owned())
    })?;
    define_name(ctx, &proto, host, "namespaceURI", |node| {
        node.and_then(|node| node.namespace_uri().map(str::to_owned))
    })?;
    define_name(ctx, &proto, host, "prefix", |node| {
        node.and_then(|node| node.prefix().map(str::to_owned))
    })
}

/// Defines the getter `key`, which reads `read` over the node of the receiver, `None` when the
/// host has freed it.
fn define_name<'js, R: rquickjs::IntoJs<'js> + 'js>(
    ctx: &Ctx<'js>,
    proto: &rquickjs::Object<'js>,
    host: &Rc<RefCell<Host>>,
    key: &str,
    read: impl Fn(Option<&VirtualNode>) -> R + 'js,
) -> JsResult<()> {
    let host = Rc::clone(host);
    define_this_getter(ctx, proto, key, move |ctx, this: Value<'js>| {
        let (node, _, _) = receiver_of(ctx, &this, &ELEMENT)?;
        Ok(read(host.borrow().tree.get(node)))
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::test_support::{HTML, MATHML, SVG, engine_with_bindings};

    /// `[tagName, localName, namespaceURI, prefix]` of the handle of the node `make` builds.
    fn names_of(make: &str) -> String {
        let (engine, _host) = engine_with_bindings();
        engine
            .eval(&format!(
                r"
                const n = __inca_native__;
                const el = n.nodeHandle({make})[0];
                JSON.stringify([el.tagName, el.localName, el.namespaceURI, el.prefix]);
                "
            ))
            .unwrap()
    }

    #[rstest]
    #[case::html("n.createNode('div')", format!(r#"["DIV","div","{HTML}",null]"#))]
    #[case::svg(
        &format!("n.createElementNS('{SVG}', 'foreignObject')[0]"),
        format!(r#"["foreignObject","foreignObject","{SVG}",null]"#)
    )]
    #[case::mathml(
        &format!("n.createElementNS('{MATHML}', 'mi')[0]"),
        format!(r#"["mi","mi","{MATHML}",null]"#)
    )]
    #[case::prefixed(
        &format!("n.createElementNS('{SVG}', 's:g')[0]"),
        format!(r#"["s:g","g","{SVG}","s"]"#)
    )]
    #[case::no_namespace("n.createElementNS(null, 'a')[0]", r#"["a","a",null,null]"#.to_owned())]
    #[case::html_ascii_upper_case(
        "n.createNode('straße')",
        format!(r#"["STRAßE","straße","{HTML}",null]"#)
    )]
    fn the_name_members_read_the_name_of_the_element(#[case] make: &str, #[case] expected: String) {
        assert_eq!(names_of(make), expected);
    }

    #[test]
    fn the_name_members_of_a_freed_element_read_the_defaults() {
        let (engine, host) = engine_with_bindings();

        let got: String = engine
            .eval(
                r"
                const n = __inca_native__;
                const id = n.createNode('div');
                globalThis.el = n.nodeHandle(id)[0];
                id;
                ",
            )
            .map(|id: u32| {
                host.borrow_mut().tree.destroy_node(id);
                engine
                    .eval::<String>(
                        "JSON.stringify([el.tagName, el.localName, el.namespaceURI, el.prefix])",
                    )
                    .unwrap()
            })
            .unwrap();

        assert_eq!(got, r#"["","",null,null]"#);
    }
}
