// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The interface objects of the nodes, with their chain and the prototype of SVG and `MathML`
//! elements, beside `DOMStringMap`, and the helpers that keep the prototypes of the interfaces and
//! of their iterators in the prototype store of the vault.
//!
//! All exist before the app runs. None can be constructed: nodes come from the methods of
//! `document`, the host has one document and no shadow roots, and a `DOMStringMap` only as the
//! `dataset` of an element.

use inca_jsenv::class::{define_class, define_global};
use inca_jsenv::{required_prototype, store_prototype};
use rquickjs::object::Property;
use rquickjs::{Ctx, Exception, Function, Object, Result as JsResult, Value};

/// The interface objects in parent order: the global, the parent and the message that a call and
/// `new` throw. A row is defined after its parent.
const NODE_INTERFACES: [(&str, Option<&str>, &str); 11] = [
    (
        "EventTarget",
        None,
        "an EventTarget cannot be constructed. listen on a node or on window",
    ),
    (
        "Node",
        Some("EventTarget"),
        "a Node cannot be constructed. create nodes with the methods of document",
    ),
    (
        "CharacterData",
        Some("Node"),
        "a CharacterData cannot be constructed. create nodes with the methods of document",
    ),
    (
        "Text",
        Some("CharacterData"),
        "a Text cannot be constructed. create it with document.createTextNode",
    ),
    (
        "Comment",
        Some("CharacterData"),
        "a Comment cannot be constructed. create it with document.createComment",
    ),
    (
        "Element",
        Some("Node"),
        "an Element cannot be constructed. create it with document.createElement",
    ),
    (
        "HTMLElement",
        Some("Element"),
        "an HTMLElement cannot be constructed. create it with document.createElement",
    ),
    (
        "DocumentFragment",
        Some("Node"),
        "a DocumentFragment cannot be constructed. create it with document.createDocumentFragment",
    ),
    (
        "ShadowRoot",
        Some("DocumentFragment"),
        "a shadow root cannot be constructed. the host has no shadow roots",
    ),
    (
        "Document",
        Some("Node"),
        "a document cannot be constructed. the host has one document",
    ),
    (
        "HTMLDocument",
        Some("Document"),
        "a document cannot be constructed. the host has one document",
    ),
];

/// The 18 constants of `Node`, the node types and the document positions.
const NODE_CONSTANTS: [(&str, i32); 18] = [
    ("ELEMENT_NODE", 1),
    ("ATTRIBUTE_NODE", 2),
    ("TEXT_NODE", 3),
    ("CDATA_SECTION_NODE", 4),
    ("ENTITY_REFERENCE_NODE", 5),
    ("ENTITY_NODE", 6),
    ("PROCESSING_INSTRUCTION_NODE", 7),
    ("COMMENT_NODE", 8),
    ("DOCUMENT_NODE", 9),
    ("DOCUMENT_TYPE_NODE", 10),
    ("DOCUMENT_FRAGMENT_NODE", 11),
    ("NOTATION_NODE", 12),
    ("DOCUMENT_POSITION_DISCONNECTED", 1),
    ("DOCUMENT_POSITION_PRECEDING", 2),
    ("DOCUMENT_POSITION_FOLLOWING", 4),
    ("DOCUMENT_POSITION_CONTAINS", 8),
    ("DOCUMENT_POSITION_CONTAINED_BY", 16),
    ("DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC", 32),
];

/// The names `interfacePrototypes` returns: the prototypes of the node interfaces that a handle
/// takes, and `ForeignElement`.
const HANDLE_PROTOTYPES: [&str; 11] = [
    "EventTarget",
    "Node",
    "CharacterData",
    "Text",
    "Comment",
    "Element",
    "HTMLElement",
    "ForeignElement",
    "DocumentFragment",
    "Document",
    "HTMLDocument",
];

/// Defines the global `name`, a class over `proto` whose every call throws a `TypeError` with
/// `message`, and returns the class.
fn define_interface<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    name: &str,
    message: &'static str,
) -> JsResult<Function<'js>> {
    let body = move |ctx: Ctx<'_>| -> JsResult<()> { Err(Exception::throw_type(&ctx, message)) };
    let class = define_class(ctx, proto, name, 0, body)?;
    define_global(ctx, name, class.clone())?;
    Ok(class)
}

/// Defines the global `name` like [`define_interface`] and keeps `proto` in the prototype store
/// under `name`.
///
/// # Errors
///
/// Returns an error if defining the global or storing the prototype fails.
pub(crate) fn define_unconstructable<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    name: &str,
    message: &'static str,
) -> JsResult<()> {
    define_interface(ctx, proto, name, message)?;
    store_prototype(ctx, name, proto)
}

/// Defines the global `name` over a new prototype that inherits the stored prototype of `parent`,
/// links the class to the class of `parent`, and keeps the prototype in the store under `name`.
fn define_node_interface(
    ctx: &Ctx<'_>,
    name: &str,
    parent: Option<&str>,
    message: &'static str,
) -> JsResult<()> {
    let proto = Object::new(ctx.clone())?;
    let class = define_interface(ctx, &proto, name, message)?;
    if let Some(parent) = parent {
        let parent_proto = required_prototype(ctx, parent)?;
        proto.set_prototype(Some(&parent_proto))?;
        let parent_class: Function = parent_proto.get("constructor")?;
        class.set_prototype(Some(&parent_class))?;
    }
    if name == "Node" {
        for (constant, value) in NODE_CONSTANTS {
            class.prop(constant, Property::from(value).enumerable())?;
            proto.prop(constant, Property::from(value).enumerable())?;
        }
    }
    store_prototype(ctx, name, &proto)
}

/// Defines the interface objects of the nodes and `DOMStringMap`, and keeps the prototype of an
/// element of the SVG or `MathML` namespace under `ForeignElement`.
///
/// # Errors
///
/// Returns an error if defining a global on `ctx` fails.
pub(crate) fn install(ctx: &Ctx<'_>) -> JsResult<()> {
    for (name, parent, message) in NODE_INTERFACES {
        define_node_interface(ctx, name, parent, message)?;
        if name == "Element" {
            let foreign = Object::new(ctx.clone())?;
            foreign.set_prototype(Some(&required_prototype(ctx, "Element")?))?;
            store_prototype(ctx, "ForeignElement", &foreign)?;
        }
    }
    define_unconstructable(
        ctx,
        &Object::new(ctx.clone())?,
        "DOMStringMap",
        "a DOMStringMap cannot be constructed. read it from dataset",
    )
}

/// The native `interfacePrototypes`: a new object with no prototype that holds the stored
/// prototype of each name of [`HANDLE_PROTOTYPES`].
///
/// The function throws if a prototype is not stored.
pub(crate) fn interface_prototypes<'js>() -> impl Fn(Ctx<'js>) -> JsResult<Object<'js>> {
    |ctx| {
        let prototypes = Object::new(ctx.clone())?;
        prototypes.set_prototype(None)?;
        for name in HANDLE_PROTOTYPES {
            prototypes.set(name, required_prototype(&ctx, name)?)?;
        }
        Ok(prototypes)
    }
}

/// The native `registerUncloneable`: registers the brand `WeakMap` of the handles, so that
/// `structuredClone` refuses a key of it.
///
/// The function throws a `TypeError` for a value that is no `WeakMap`.
pub(crate) fn register_uncloneable<'js>() -> impl Fn(Ctx<'js>, Value<'js>) -> JsResult<()> {
    |ctx, map| inca_jsenv::register_uncloneable(&ctx, &map)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::test_support::engine_with_bindings;
    use crate::tests::{dispatcher_with_engine, eval_json};
    use inca_jsenv::required_prototype;
    use rquickjs::{Function, Object};
    use rstest::rstest;

    const EVENT_TARGET: &str =
        "an EventTarget cannot be constructed. listen on a node or on window";
    const NODE: &str = "a Node cannot be constructed. create nodes with the methods of document";
    const CHARACTER_DATA: &str =
        "a CharacterData cannot be constructed. create nodes with the methods of document";
    const TEXT: &str = "a Text cannot be constructed. create it with document.createTextNode";
    const COMMENT: &str = "a Comment cannot be constructed. create it with document.createComment";
    const ELEMENT: &str = "an Element cannot be constructed. create it with document.createElement";
    const HTML_ELEMENT: &str =
        "an HTMLElement cannot be constructed. create it with document.createElement";
    const FRAGMENT: &str = concat!(
        "a DocumentFragment cannot be constructed. ",
        "create it with document.createDocumentFragment"
    );
    const DOCUMENT: &str = "a document cannot be constructed. the host has one document";
    const SHADOW_ROOT: &str = "a shadow root cannot be constructed. the host has no shadow roots";
    const TOKEN_LIST: &str = "a token list cannot be constructed. read it from classList";
    const STYLE: &str =
        "a style declaration cannot be constructed. read it from the style of an element";
    const DOMSTRING_MAP: &str = "a DOMStringMap cannot be constructed. read it from dataset";

    #[rstest]
    #[case::event_target_new("new EventTarget()", EVENT_TARGET)]
    #[case::event_target_call("EventTarget()", EVENT_TARGET)]
    #[case::node_new("new Node()", NODE)]
    #[case::node_call("Node()", NODE)]
    #[case::character_data_new("new CharacterData()", CHARACTER_DATA)]
    #[case::character_data_call("CharacterData()", CHARACTER_DATA)]
    #[case::text_new("new Text()", TEXT)]
    #[case::text_call("Text()", TEXT)]
    #[case::comment_new("new Comment()", COMMENT)]
    #[case::comment_call("Comment()", COMMENT)]
    #[case::element_new("new Element()", ELEMENT)]
    #[case::element_call("Element()", ELEMENT)]
    #[case::html_element_new("new HTMLElement()", HTML_ELEMENT)]
    #[case::html_element_call("HTMLElement()", HTML_ELEMENT)]
    #[case::fragment_new("new DocumentFragment()", FRAGMENT)]
    #[case::fragment_call("DocumentFragment()", FRAGMENT)]
    #[case::document_new("new Document()", DOCUMENT)]
    #[case::document_call("Document()", DOCUMENT)]
    #[case::html_document_new("new HTMLDocument()", DOCUMENT)]
    #[case::html_document_call("HTMLDocument()", DOCUMENT)]
    #[case::shadow_root_new("new ShadowRoot()", SHADOW_ROOT)]
    #[case::shadow_root_call("ShadowRoot()", SHADOW_ROOT)]
    #[case::token_list_new("new DOMTokenList()", TOKEN_LIST)]
    #[case::token_list_call("DOMTokenList()", TOKEN_LIST)]
    #[case::style_new("new CSSStyleDeclaration()", STYLE)]
    #[case::style_call("CSSStyleDeclaration()", STYLE)]
    #[case::dom_string_map_new("new DOMStringMap()", DOMSTRING_MAP)]
    #[case::dom_string_map_call("DOMStringMap()", DOMSTRING_MAP)]
    fn constructing_an_interface_throws_a_type_error(#[case] call: &str, #[case] message: &str) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                (() => {{
                    try {{
                        {call};
                        return 'ran';
                    }} catch (err) {{
                        return [err instanceof TypeError, err.message];
                    }}
                }})()
                "
            ),
        );
        assert_eq!(got, serde_json::json!([true, message]));
    }

    #[rstest]
    #[case::event_target("EventTarget")]
    #[case::node("Node")]
    #[case::character_data("CharacterData")]
    #[case::text("Text")]
    #[case::comment("Comment")]
    #[case::element("Element")]
    #[case::html_element("HTMLElement")]
    #[case::fragment("DocumentFragment")]
    #[case::document("Document")]
    #[case::html_document("HTMLDocument")]
    #[case::shadow_root("ShadowRoot")]
    #[case::token_list("DOMTokenList")]
    #[case::style("CSSStyleDeclaration")]
    #[case::dom_string_map("DOMStringMap")]
    fn an_interface_has_length_zero(#[case] name: &str) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        assert_eq!(
            eval_json(&dispatcher, &format!("{name}.length")),
            serde_json::json!(0)
        );
    }

    #[rstest]
    #[case::node("Node", "EventTarget")]
    #[case::character_data("CharacterData", "Node")]
    #[case::text("Text", "CharacterData")]
    #[case::comment("Comment", "CharacterData")]
    #[case::element("Element", "Node")]
    #[case::html_element("HTMLElement", "Element")]
    #[case::fragment("DocumentFragment", "Node")]
    #[case::shadow_root("ShadowRoot", "DocumentFragment")]
    #[case::document("Document", "Node")]
    #[case::html_document("HTMLDocument", "Document")]
    fn an_interface_prototype_inherits_the_parent_prototype(
        #[case] child: &str,
        #[case] parent: &str,
    ) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let got = eval_json(
            &dispatcher,
            &format!("Object.getPrototypeOf({child}.prototype) === {parent}.prototype"),
        );
        assert_eq!(got, serde_json::json!(true));
    }

    #[rstest]
    #[case::node("Node", "EventTarget")]
    #[case::character_data("CharacterData", "Node")]
    #[case::text("Text", "CharacterData")]
    #[case::comment("Comment", "CharacterData")]
    #[case::element("Element", "Node")]
    #[case::html_element("HTMLElement", "Element")]
    #[case::fragment("DocumentFragment", "Node")]
    #[case::shadow_root("ShadowRoot", "DocumentFragment")]
    #[case::document("Document", "Node")]
    #[case::html_document("HTMLDocument", "Document")]
    fn an_interface_inherits_the_parent_interface(#[case] child: &str, #[case] parent: &str) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let got = eval_json(
            &dispatcher,
            &format!("Object.getPrototypeOf({child}) === {parent}"),
        );
        assert_eq!(got, serde_json::json!(true));
    }

    #[test]
    fn event_target_prototype_inherits_object_prototype() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let got = eval_json(
            &dispatcher,
            "Object.getPrototypeOf(EventTarget.prototype) === Object.prototype",
        );
        assert_eq!(got, serde_json::json!(true));
    }

    #[test]
    fn event_target_inherits_function_prototype() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let got = eval_json(
            &dispatcher,
            "Object.getPrototypeOf(EventTarget) === Function.prototype",
        );
        assert_eq!(got, serde_json::json!(true));
    }

    #[rstest]
    #[case::event_target("EventTarget")]
    #[case::node("Node")]
    #[case::character_data("CharacterData")]
    #[case::text("Text")]
    #[case::comment("Comment")]
    #[case::element("Element")]
    #[case::html_element("HTMLElement")]
    #[case::fragment("DocumentFragment")]
    #[case::document("Document")]
    #[case::html_document("HTMLDocument")]
    #[case::shadow_root("ShadowRoot")]
    fn a_prototype_has_the_string_tag_of_its_interface(#[case] name: &str) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let got = eval_json(
            &dispatcher,
            &format!("Object.prototype.toString.call({name}.prototype)"),
        );
        assert_eq!(got, serde_json::json!(format!("[object {name}]")));
    }

    #[rstest]
    #[case::event_target("EventTarget")]
    #[case::node("Node")]
    #[case::character_data("CharacterData")]
    #[case::text("Text")]
    #[case::comment("Comment")]
    #[case::element("Element")]
    #[case::html_element("HTMLElement")]
    #[case::fragment("DocumentFragment")]
    #[case::document("Document")]
    #[case::html_document("HTMLDocument")]
    #[case::shadow_root("ShadowRoot")]
    fn a_prototype_constructor_is_its_interface(#[case] name: &str) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let got = eval_json(
            &dispatcher,
            &format!("{name}.prototype.constructor === {name}"),
        );
        assert_eq!(got, serde_json::json!(true));
    }

    #[rstest]
    #[case::class("Node")]
    #[case::prototype("Node.prototype")]
    fn node_holds_the_eighteen_constants(#[case] owner: &str) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let got = eval_json(
            &dispatcher,
            &format!("Object.fromEntries(Object.entries({owner}))"),
        );
        assert_eq!(
            got,
            serde_json::json!({
                "ELEMENT_NODE": 1,
                "ATTRIBUTE_NODE": 2,
                "TEXT_NODE": 3,
                "CDATA_SECTION_NODE": 4,
                "ENTITY_REFERENCE_NODE": 5,
                "ENTITY_NODE": 6,
                "PROCESSING_INSTRUCTION_NODE": 7,
                "COMMENT_NODE": 8,
                "DOCUMENT_NODE": 9,
                "DOCUMENT_TYPE_NODE": 10,
                "DOCUMENT_FRAGMENT_NODE": 11,
                "NOTATION_NODE": 12,
                "DOCUMENT_POSITION_DISCONNECTED": 1,
                "DOCUMENT_POSITION_PRECEDING": 2,
                "DOCUMENT_POSITION_FOLLOWING": 4,
                "DOCUMENT_POSITION_CONTAINS": 8,
                "DOCUMENT_POSITION_CONTAINED_BY": 16,
                "DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC": 32,
            })
        );
    }

    #[test]
    fn a_node_constant_is_read_only_enumerable_and_non_configurable() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let got = eval_json(
            &dispatcher,
            "Object.getOwnPropertyDescriptor(Node, 'ELEMENT_NODE')",
        );
        assert_eq!(
            got,
            serde_json::json!({
                "value": 1,
                "writable": false,
                "enumerable": true,
                "configurable": false,
            })
        );
    }

    #[rstest]
    #[case::event_target("EventTarget")]
    #[case::node("Node")]
    #[case::character_data("CharacterData")]
    #[case::text("Text")]
    #[case::comment("Comment")]
    #[case::element("Element")]
    #[case::html_element("HTMLElement")]
    #[case::fragment("DocumentFragment")]
    #[case::document("Document")]
    #[case::html_document("HTMLDocument")]
    #[case::shadow_root("ShadowRoot")]
    #[case::token_list("DOMTokenList")]
    #[case::style("CSSStyleDeclaration")]
    #[case::dom_string_map("DOMStringMap")]
    fn the_store_keeps_the_prototype_of_every_interface(#[case] name: &str) {
        let (engine, _host) = engine_with_bindings();

        let got = engine.with(|ctx| {
            let proto = required_prototype(&ctx, name).unwrap();
            let class: Function = proto.get("constructor").unwrap();
            class.get::<_, String>("name").unwrap()
        });

        assert_eq!(got, name);
    }

    #[test]
    fn the_foreign_element_prototype_inherits_element() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let got = eval_json(
            &dispatcher,
            r"
            Object.getPrototypeOf(__inca_native__.interfacePrototypes().ForeignElement) ===
                Element.prototype
            ",
        );
        assert_eq!(got, serde_json::json!(true));
    }

    #[test]
    fn the_foreign_element_prototype_has_no_own_key() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let got = eval_json(
            &dispatcher,
            "Reflect.ownKeys(__inca_native__.interfacePrototypes().ForeignElement)",
        );
        assert_eq!(got, serde_json::json!([]));
    }

    #[test]
    fn no_global_names_the_foreign_element_prototype() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let got = eval_json(&dispatcher, "typeof ForeignElement");
        assert_eq!(got, serde_json::json!("undefined"));
    }

    #[rstest]
    #[case::event_target("EventTarget")]
    #[case::node("Node")]
    #[case::character_data("CharacterData")]
    #[case::text("Text")]
    #[case::comment("Comment")]
    #[case::element("Element")]
    #[case::html_element("HTMLElement")]
    #[case::foreign_element("ForeignElement")]
    #[case::fragment("DocumentFragment")]
    #[case::document("Document")]
    #[case::html_document("HTMLDocument")]
    fn interface_prototypes_gives_the_stored_prototype(#[case] name: &str) {
        let (engine, _host) = engine_with_bindings();

        let same = engine.with(|ctx| {
            let native: Object = ctx.globals().get("__inca_native__").unwrap();
            let read: Function = native.get("interfacePrototypes").unwrap();
            let got: Object = read.call(()).unwrap();
            got.get::<_, Object>(name).unwrap() == required_prototype(&ctx, name).unwrap()
        });

        assert!(same);
    }

    #[test]
    fn interface_prototypes_gives_a_new_object_at_each_call() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let got = eval_json(
            &dispatcher,
            "__inca_native__.interfacePrototypes() !== __inca_native__.interfacePrototypes()",
        );
        assert_eq!(got, serde_json::json!(true));
    }

    #[test]
    fn interface_prototypes_gives_an_object_with_no_prototype() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let got = eval_json(
            &dispatcher,
            "Object.getPrototypeOf(__inca_native__.interfacePrototypes())",
        );
        assert_eq!(got, serde_json::Value::Null);
    }

    #[test]
    fn interface_prototypes_holds_the_eleven_names_alone() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let got = eval_json(
            &dispatcher,
            "Object.keys(__inca_native__.interfacePrototypes()).sort()",
        );
        assert_eq!(
            got,
            serde_json::json!([
                "CharacterData",
                "Comment",
                "Document",
                "DocumentFragment",
                "Element",
                "EventTarget",
                "ForeignElement",
                "HTMLDocument",
                "HTMLElement",
                "Node",
                "Text",
            ])
        );
    }
}
