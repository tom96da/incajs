// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The class `NodeHandle`, whose instances are the node handles of core, the rules that check the
//! receiver of a host member, and the native `nodeHandle`.
//!
//! An instance holds the node id, the kind and the space of the node. It has the stored prototype
//! of its interface and the own property `__node_id`.

use std::cell::RefCell;
use std::rc::Rc;

// The derive of `JsLifetime` on `NodeHandle`.
#[allow(clippy::disallowed_types)]
use rquickjs::JsLifetime;
use rquickjs::object::Property;
use rquickjs::{Array, Class, Ctx, IntoJs, Object, Result as JsResult, Value};

use inca_jsenv::class::{plain_class, receiver_named};
use inca_jsenv::required_prototype;
use inca_node::{NodeId, NodeKind, VirtualNode};

use super::Natives;
use super::convert::{JsNodeId, or_null};
use super::host::Host;

/// The SVG namespace.
const SVG: &str = "http://www.w3.org/2000/svg";
/// The `MathML` namespace.
const MATHML: &str = "http://www.w3.org/1998/Math/MathML";

/// Which namespace class an element is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Space {
    /// The HTML namespace.
    Html,
    /// The SVG or `MathML` namespace.
    Foreign,
    /// Any other namespace, or none. Every node that is no element has it.
    Other,
}

impl Space {
    /// The space of `node`.
    fn of(node: &VirtualNode) -> Self {
        if !node.is_element() {
            return Self::Other;
        }
        match node.namespace_uri() {
            _ if node.in_html_namespace() => Self::Html,
            Some(SVG | MATHML) => Self::Foreign,
            _ => Self::Other,
        }
    }

    fn bit(self) -> u8 {
        match self {
            Self::Html => 1,
            Self::Foreign => 2,
            Self::Other => 4,
        }
    }
}

/// The instance of `NodeHandle`. It holds the node id and no JS value.
// `NodeHandle` holds plain data and no JS value, so the derive is sound.
#[allow(clippy::disallowed_macros)]
#[derive(JsLifetime)]
pub(super) struct NodeHandle {
    node: NodeId,
    kind: NodeKind,
    space: Space,
}

plain_class!(NodeHandle, "NodeHandle");

/// What a member needs of its receiver: the kinds and, for an element, the spaces it takes.
pub(super) struct Rule {
    kinds: u8,
    spaces: u8,
    /// The interface name of the error message.
    name: &'static str,
}

fn kind_bit(kind: NodeKind) -> u8 {
    match kind {
        NodeKind::Element => 1,
        NodeKind::Text => 2,
        NodeKind::Comment => 4,
        NodeKind::Fragment => 8,
        NodeKind::Document => 16,
    }
}

/// The receiver of an `Element` member: an element of any namespace.
pub(super) const ELEMENT: Rule = Rule {
    kinds: 1,
    spaces: 7,
    name: "Element",
};

/// The node, kind and space of the instance `this`, which has to fit `rule`.
///
/// A node of another kind than an element passes every space.
///
/// # Errors
///
/// Returns a `TypeError` if `this` is no `NodeHandle` or its kind or space does not fit `rule`.
pub(super) fn receiver_of<'js>(
    ctx: &Ctx<'js>,
    this: &Value<'js>,
    rule: &Rule,
) -> JsResult<(NodeId, NodeKind, Space)> {
    let handle = receiver_named::<NodeHandle>(ctx, this, rule.name)?;
    let (node, kind, space) = {
        let handle = handle.borrow();
        (handle.node, handle.kind, handle.space)
    };
    let space_fits = kind != NodeKind::Element || rule.spaces & space.bit() != 0;
    if rule.kinds & kind_bit(kind) == 0 || !space_fits {
        // The error of a value that is no instance names the interface the same way.
        receiver_named::<NodeHandle>(ctx, &Value::new_undefined(ctx.clone()), rule.name)?;
    }
    Ok((node, kind, space))
}

/// The name of the stored prototype a node takes.
fn prototype_name(kind: NodeKind, space: Space) -> &'static str {
    match (kind, space) {
        (NodeKind::Element, Space::Html) => "HTMLElement",
        (NodeKind::Element, Space::Foreign) => "ForeignElement",
        (NodeKind::Element, Space::Other) => "Element",
        (NodeKind::Text, _) => "Text",
        (NodeKind::Comment, _) => "Comment",
        (NodeKind::Fragment, _) => "DocumentFragment",
        (NodeKind::Document, _) => "HTMLDocument",
    }
}

/// `"input"` for an HTML `input`, `"template"` for an HTML `template`, `None` for any other node.
fn special_of(node: &VirtualNode) -> Option<&'static str> {
    if node.is_html("input") {
        Some("input")
    } else if node.is_html("template") {
        Some("template")
    } else {
        None
    }
}

/// `nodeHandle`.
pub(super) fn install_handle(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("nodeHandle", node_handle)
}

/// A new handle of the node `id`: `[handle, parented, special]`, where `parented` tells whether the
/// node has a parent and `special` is `"input"`, `"template"` or `null`. An id the tree does not
/// have gives `null`.
fn node_handle<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id)| {
        let host = host.borrow();
        let Some(node) = host.tree.get(node_id) else {
            return Ok(Value::new_null(ctx));
        };
        let (kind, space) = (node.kind(), Space::of(node));
        let proto = required_prototype(&ctx, prototype_name(kind, space))?;
        let handle = Class::instance_proto(
            NodeHandle {
                node: node_id,
                kind,
                space,
            },
            proto,
        )?;
        let object: Object = handle.into_inner();
        object.prop("__node_id", Property::from(node_id))?;
        let result = Array::new(ctx.clone())?;
        result.set(0, object)?;
        result.set(1, node.parent().is_some())?;
        result.set(2, or_null(&ctx, special_of(node))?)?;
        result.into_js(&ctx)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::test_support::{MATHML, SVG, engine_with_bindings, error_name};

    /// The name of the interface of the prototype of the handle of the node `make` builds.
    fn interface_of(make: &str) -> String {
        let (engine, _host) = engine_with_bindings();
        engine
            .eval(&format!(
                r"
                const n = __inca_native__;
                const id = {make};
                const handle = n.nodeHandle(id)[0];
                const protos = n.interfacePrototypes();
                Object.keys(protos).find((name) => protos[name] === Object.getPrototypeOf(handle));
                "
            ))
            .unwrap()
    }

    #[rstest]
    #[case::html("n.createNode('div')", "HTMLElement")]
    #[case::svg(&format!("n.createElementNS('{SVG}', 'g')[0]"), "ForeignElement")]
    #[case::mathml(&format!("n.createElementNS('{MATHML}', 'mi')[0]"), "ForeignElement")]
    #[case::other_namespace("n.createElementNS('urn:x', 'a')[0]", "Element")]
    #[case::no_namespace("n.createElementNS(null, 'a')[0]", "Element")]
    #[case::text("n.createNode('#text')", "Text")]
    #[case::comment("n.createNode('#comment')", "Comment")]
    #[case::fragment("n.createNode('#fragment')", "DocumentFragment")]
    #[case::document("n.documentNodeId()", "HTMLDocument")]
    #[case::document_copy("n.cloneNode(n.documentNodeId(), false)", "HTMLDocument")]
    fn a_handle_takes_the_prototype_of_its_kind_and_space(
        #[case] make: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(interface_of(make), expected);
    }

    #[test]
    fn the_node_id_of_a_handle_is_read_only_hidden_and_fixed() {
        let (engine, _host) = engine_with_bindings();

        let got: String = engine
            .eval(
                r"
                const n = __inca_native__;
                const id = n.createNode('div');
                const d = Object.getOwnPropertyDescriptor(n.nodeHandle(id)[0], '__node_id');
                [d.value === id, d.writable, d.enumerable, d.configurable].join();
                ",
            )
            .unwrap();

        assert_eq!(got, "true,false,false,false");
    }

    #[test]
    fn a_handle_of_an_unknown_id_is_null() {
        let (engine, _host) = engine_with_bindings();

        let got: bool = engine
            .eval("__inca_native__.nodeHandle(999) === null")
            .unwrap();

        assert!(got);
    }

    #[rstest]
    #[case::created(
        r"
        const id = n.createNode('div');
        ",
        false
    )]
    #[case::child(
        r"
        const id = n.createNode('div');
        n.appendChild(n.rootNodeId(), id);
        ",
        true
    )]
    #[case::removed(
        r"
        const id = n.createNode('div');
        n.appendChild(n.rootNodeId(), id);
        n.removeChild(n.rootNodeId(), id);
        ",
        false
    )]
    fn a_handle_reports_whether_its_node_has_a_parent(#[case] setup: &str, #[case] expected: bool) {
        let (engine, _host) = engine_with_bindings();

        let got: bool = engine
            .eval(&format!(
                r"
                const n = __inca_native__;
                {setup}
                n.nodeHandle(id)[1];
                "
            ))
            .unwrap();

        assert_eq!(got, expected);
    }

    #[rstest]
    #[case::input("n.createNode('input')", "input")]
    #[case::template("n.createNode('template')", "template")]
    #[case::svg_template(&format!("n.createElementNS('{SVG}', 'template')[0]"), "null")]
    #[case::div("n.createNode('div')", "null")]
    fn a_handle_reports_the_special_element_it_is(#[case] make: &str, #[case] expected: &str) {
        let (engine, _host) = engine_with_bindings();

        let got: String = engine
            .eval(&format!(
                r"
                const n = __inca_native__;
                String(n.nodeHandle({make})[2]);
                "
            ))
            .unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn structured_clone_refuses_a_handle() {
        let (engine, _host) = engine_with_bindings();

        let got = error_name(
            &engine,
            r"
            const n = __inca_native__;
            structuredClone(n.nodeHandle(n.createNode('div'))[0]);
            ",
        );

        assert_eq!(got, "DataCloneError");
    }

    #[test]
    fn structured_clone_copies_a_plain_object_of_the_shape_of_a_handle() {
        let (engine, _host) = engine_with_bindings();

        let got: String = engine
            .eval(
                r"
                const n = __inca_native__;
                const handle = n.nodeHandle(n.createNode('div'))[0];
                const plain = Object.create(Object.getPrototypeOf(handle));
                Object.defineProperty(plain, '__node_id', { value: 1, enumerable: true });
                typeof structuredClone(plain);
                ",
            )
            .unwrap();

        assert_eq!(got, "object");
    }

    #[test]
    fn a_member_called_on_a_forged_object_throws_the_receiver_error() {
        let (engine, _host) = engine_with_bindings();

        let got = error_name(
            &engine,
            r"
            const n = __inca_native__;
            const forged = Object.create(n.interfacePrototypes().HTMLElement);
            Object.defineProperty(forged, '__node_id', { value: n.createNode('div') });
            const element = n.interfacePrototypes().Element;
            Object.getOwnPropertyDescriptor(element, 'tagName').get.call(forged);
            ",
        );

        assert_eq!(got, "TypeError");
    }

    #[rstest]
    #[case::text("n.createNode('#text')")]
    #[case::comment("n.createNode('#comment')")]
    #[case::fragment("n.createNode('#fragment')")]
    #[case::document("n.documentNodeId()")]
    fn an_element_member_called_on_a_handle_of_another_kind_throws_the_receiver_error(
        #[case] make: &str,
    ) {
        let (engine, _host) = engine_with_bindings();

        let got = error_name(
            &engine,
            &format!(
                r"
                const n = __inca_native__;
                const handle = n.nodeHandle({make})[0];
                const element = n.interfacePrototypes().Element;
                Object.getOwnPropertyDescriptor(element, 'tagName').get.call(handle);
                "
            ),
        );

        assert_eq!(got, "TypeError");
    }
}
