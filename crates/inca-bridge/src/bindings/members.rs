// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The members the host defines on the stored prototypes of the node interfaces. Each takes its
//! receiver through the rules of `node/handle.rs`. A node the host no longer has is a freed node: a
//! read gives the default of its type, and a write or an action does nothing.

use std::rc::Rc;

use rquickjs::{Ctx, IntoJs, Object, Result as JsResult, Value};

use inca_jsenv::class::{define_this_accessor, define_this_getter, define_this_method};
use inca_node::{NodeId, NodeKind};

use super::host::Host;
use crate::node::handle::{Rule, receiver_of};

/// The host a member reads and writes.
pub(crate) type Shared = Rc<Host>;

/// Defines the getter `key` of `proto`. `read` gets the node and the kind of the receiver.
pub(crate) fn getter<'js, R: IntoJs<'js> + 'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    host: &Shared,
    key: &str,
    rule: &'static Rule,
    read: impl Fn(&Ctx<'js>, &Shared, NodeId, NodeKind) -> JsResult<R> + 'js,
) -> JsResult<()> {
    let host = Rc::clone(host);
    define_this_getter(ctx, proto, key, move |ctx, this: Value<'js>| {
        let (node, kind, _) = receiver_of(ctx, &this, rule)?;
        read(ctx, &host, node, kind)
    })
}

/// Defines the accessor `key` of `proto`. `write` gets the assigned value as it came.
pub(crate) fn accessor<'js, R: IntoJs<'js> + 'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    host: &Shared,
    key: &str,
    rule: &'static Rule,
    read: impl Fn(&Ctx<'js>, &Shared, NodeId, NodeKind) -> JsResult<R> + 'js,
    write: impl Fn(&Ctx<'js>, &Shared, NodeId, NodeKind, Value<'js>) -> JsResult<()> + 'js,
) -> JsResult<()> {
    let (reader, writer) = (Rc::clone(host), Rc::clone(host));
    define_this_accessor(
        ctx,
        proto,
        key,
        move |ctx, this: Value<'js>| {
            let (node, kind, _) = receiver_of(ctx, &this, rule)?;
            read(ctx, &reader, node, kind)
        },
        move |ctx, this: Value<'js>, value: Value<'js>| {
            let (node, kind, _) = receiver_of(ctx, &this, rule)?;
            write(ctx, &writer, node, kind, value)
        },
    )
}

/// Defines the method `name` of `proto` with `length` parameters. `run` gets the arguments of the
/// call.
pub(crate) fn method<'js, R: IntoJs<'js> + 'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    host: &Shared,
    name: &str,
    length: usize,
    rule: &'static Rule,
    run: impl Fn(&Ctx<'js>, &Shared, NodeId, NodeKind, &[Value<'js>]) -> JsResult<R> + 'js,
) -> JsResult<()> {
    let host = Rc::clone(host);
    define_this_method(ctx, proto, name, length, move |ctx, this, args| {
        let (node, kind, _) = receiver_of(ctx, &this, rule)?;
        run(ctx, &host, node, kind, args)
    })
    .map(drop)
}

/// Defines the host members on the prototypes `interfaces::install` stored.
pub(super) fn install_members(ctx: &Ctx<'_>, host: &Rc<Host>) -> JsResult<()> {
    crate::node::install_members(ctx, host)?;
    crate::element::install_members(ctx, host)?;
    crate::element::install_markup_members(ctx, host)?;
    crate::element::install_layout_members(ctx, host)?;
    crate::element::install_semantics_members(ctx, host)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;
    use serde_json::json;

    use crate::test_support::{engine_with_bindings, json_of};

    #[rstest]
    #[case::node("Node", &["nodeType", "nodeName", "nodeValue", "textContent", "isConnected", "hasChildNodes"])]
    #[case::character_data("CharacterData", &["data", "length", "substringData", "appendData", "insertData", "deleteData", "replaceData"])]
    #[case::text("Text", &["wholeText"])]
    #[case::element("Element", &["id", "className", "tagName", "localName", "namespaceURI", "prefix", "getAttribute", "setAttribute", "hasAttribute", "removeAttribute", "getAttributeNames", "matches", "innerHTML", "outerHTML", "getBoundingClientRect", "clientWidth", "clientHeight", "clientLeft", "clientTop", "scrollLeft", "scrollTop", "scrollWidth", "scrollHeight", "scrollTo", "scrollBy", "scrollIntoView", "setPointerCapture", "releasePointerCapture", "hasPointerCapture", "childElementCount", "role", "ariaAtomic", "ariaAutoComplete", "ariaBrailleLabel", "ariaBrailleRoleDescription", "ariaBusy", "ariaChecked", "ariaColCount", "ariaColIndex", "ariaColIndexText", "ariaColSpan", "ariaCurrent", "ariaDescription", "ariaDisabled", "ariaExpanded", "ariaHasPopup", "ariaHidden", "ariaInvalid", "ariaKeyShortcuts", "ariaLabel", "ariaLevel", "ariaLive", "ariaModal", "ariaMultiLine", "ariaMultiSelectable", "ariaOrientation", "ariaPlaceholder", "ariaPosInSet", "ariaPressed", "ariaReadOnly", "ariaRelevant", "ariaRequired", "ariaRoleDescription", "ariaRowCount", "ariaRowIndex", "ariaRowIndexText", "ariaRowSpan", "ariaSelected", "ariaSetSize", "ariaSort", "ariaValueMax", "ariaValueMin", "ariaValueNow", "ariaValueText"])]
    #[case::html_element("HTMLElement", &["click", "offsetWidth", "offsetHeight", "offsetLeft", "offsetTop", "focus", "blur", "tabIndex"])]
    #[case::foreign_element("ForeignElement", &["focus", "blur", "tabIndex"])]
    #[case::document("Document", &["childElementCount"])]
    #[case::fragment("DocumentFragment", &["childElementCount"])]
    fn a_prototype_holds_its_host_members(#[case] name: &str, #[case] members: &[&str]) {
        let (engine, _host) = engine_with_bindings();

        let got = json_of(
            &engine,
            &format!(
                "Reflect.ownKeys(__inca_native__.interfacePrototypes().{name}).filter((k) => typeof k === 'string' && k !== 'constructor' && k !== k.toUpperCase())"
            ),
        );

        let mut want: Vec<&str> = members.to_vec();
        want.sort_unstable();
        let mut got: Vec<String> = serde_json::from_value(got).unwrap();
        got.sort_unstable();
        assert_eq!(got, want);
    }

    /// The `length` of each host method.
    #[rstest]
    #[case::has_child_nodes("Node", "hasChildNodes", 0)]
    #[case::substring_data("CharacterData", "substringData", 2)]
    #[case::append_data("CharacterData", "appendData", 1)]
    #[case::insert_data("CharacterData", "insertData", 2)]
    #[case::delete_data("CharacterData", "deleteData", 2)]
    #[case::replace_data("CharacterData", "replaceData", 3)]
    #[case::get_attribute("Element", "getAttribute", 1)]
    #[case::set_attribute("Element", "setAttribute", 2)]
    #[case::has_attribute("Element", "hasAttribute", 1)]
    #[case::remove_attribute("Element", "removeAttribute", 1)]
    #[case::get_attribute_names("Element", "getAttributeNames", 0)]
    #[case::matches("Element", "matches", 1)]
    #[case::get_bounding_client_rect("Element", "getBoundingClientRect", 0)]
    #[case::scroll_to("Element", "scrollTo", 0)]
    #[case::scroll_by("Element", "scrollBy", 0)]
    #[case::scroll_into_view("Element", "scrollIntoView", 0)]
    #[case::set_pointer_capture("Element", "setPointerCapture", 1)]
    #[case::click("HTMLElement", "click", 0)]
    #[case::focus("HTMLElement", "focus", 0)]
    #[case::blur("ForeignElement", "blur", 0)]
    fn a_method_has_its_length(#[case] proto: &str, #[case] name: &str, #[case] length: u32) {
        let (engine, _host) = engine_with_bindings();

        let got = json_of(
            &engine,
            &format!("__inca_native__.interfacePrototypes().{proto}.{name}.length"),
        );

        assert_eq!(got, json!(length));
    }

    #[test]
    fn a_mixin_member_is_defined_once_for_each_interface() {
        let (engine, _host) = engine_with_bindings();

        let got = json_of(
            &engine,
            r"
            (() => {
                const p = __inca_native__.interfacePrototypes();
                const own = (o, k) => Object.getOwnPropertyDescriptor(o, k).value;
                return own(p.HTMLElement, 'focus') !== own(p.ForeignElement, 'focus');
            })()
            ",
        );

        assert_eq!(got, json!(true));
    }
}
