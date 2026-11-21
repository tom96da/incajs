// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use inca_node::{AttributeValue, ParsedHtml};
use inca_style::set_style_text;

use super::super::attributes::print_style_warning;
use super::super::host::Host;

/// The steps that follow a parse once its nodes are in place: applies each `style` attribute as
/// style props and prints its warnings, gives an `input` the typed text of its `value` attribute
/// and frees the result fragment, which is empty.
pub(super) fn finish(host: &mut Host, parsed: &ParsedHtml) {
    for node in &parsed.nodes {
        let style = host
            .tree
            .get(node.id)
            .and_then(|element| element.attributes().get("style"))
            .and_then(AttributeValue::as_str)
            .map(str::to_owned);
        // The node was read one statement ago, so the write cannot find it missing.
        let warnings = style.and_then(|style| set_style_text(&mut host.tree, node.id, &style).ok());
        for line in warnings.into_iter().flatten() {
            print_style_warning(host, node.id, &line);
        }
    }
    for node in &parsed.nodes {
        let typed = host
            .tree
            .get(node.id)
            .filter(|input| input.is_html("input"))
            .and_then(|input| input.attributes().get("value"))
            .and_then(AttributeValue::as_str)
            .map(str::to_owned);
        if let Some(typed) = typed {
            host.inputs
                .borrow_mut()
                .entry(node.id)
                .or_default()
                .set_value(&typed);
        }
    }
    host.tree.destroy_node(parsed.fragment);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::cell::RefCell;
    use std::rc::Rc;

    use inca_jsenv::Engine;
    use rstest::rstest;

    use crate::Host;
    use crate::test_support::engine_with_bindings;

    /// `host` is a `div` under the root.
    const SETUP: &str = r"
        const n = __inca_native__;
        const host = n.createNode('div');
        n.appendChild(n.rootNodeId(), host);
        ";

    fn engine() -> (Engine, Rc<RefCell<Host>>) {
        let (engine, host) = engine_with_bindings();
        engine.eval::<()>(SETUP).unwrap();
        (engine, host)
    }

    /// Parses `markup` into `host` and returns the id of its first child.
    fn parse(engine: &Engine, markup: &str) -> u32 {
        engine
            .eval(&format!(
                r"
                n.setInnerHtml(host, '{markup}');
                n.childNodeIds(host)[0];
                "
            ))
            .unwrap()
    }

    /// Collects the warnings the host prints.
    fn collect_warnings(host: &Rc<RefCell<Host>>) -> Rc<RefCell<Vec<String>>> {
        let lines = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&lines);
        host.borrow_mut().warn = Rc::new(move |line| sink.borrow_mut().push(line.to_owned()));
        lines
    }

    #[rstest]
    #[case::html("<input value=a>", "n.childNodeIds(host)[0]", true)]
    #[case::svg(
        "<svg><input value=a></svg>",
        "n.childNodeIds(n.childNodeIds(host)[0])[0]",
        false
    )]
    fn a_parsed_input_takes_the_value_attribute_as_typed_text_in_html_only(
        #[case] markup: &str,
        #[case] input: &str,
        #[case] typed: bool,
    ) {
        let (engine, host) = engine();

        let id: u32 = engine
            .eval(&format!(
                r"
                n.setInnerHtml(host, '{markup}');
                {input};
                "
            ))
            .unwrap();

        assert_eq!(host.borrow().inputs.borrow().contains_key(&id), typed);
    }

    #[test]
    fn style_warnings_print_once_for_each_node_and_line() {
        let (engine, host) = engine();
        let lines = collect_warnings(&host);

        engine
            .eval::<()>(
                r#"
                n.setInnerHtml(host, '<p style="bogus:1;bogus:2"><i style="bogus:1"></i></p>');
                "#,
            )
            .unwrap();

        let lines = lines.borrow();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].contains("(p)"), "{lines:?}");
        assert!(lines[1].contains("(i)"), "{lines:?}");
    }

    #[test]
    fn a_parsed_style_attribute_applies_as_style_props() {
        let (engine, host) = engine();

        let id = parse(&engine, r#"<div style="COLOR: #f00">"#);

        let host = host.borrow();
        assert_eq!(
            host.tree.get(id).unwrap().style_props().get("text_color"),
            Some(&"#f00".into())
        );
    }

    #[test]
    fn a_parsed_style_attribute_keeps_its_source_position() {
        let (engine, host) = engine();

        let id = parse(&engine, r#"<div style="width: 1px" id="a">"#);

        let host = host.borrow();
        let keys: Vec<&String> = host.tree.get(id).unwrap().attributes().keys().collect();
        assert_eq!(keys, ["style", "id"]);
    }

    #[test]
    fn a_parsed_style_attribute_is_stored_as_the_derived_text() {
        let (engine, host) = engine();

        let id = parse(&engine, r#"<div style="WIDTH:1px; bogus: 1; height:2PX">"#);

        let host = host.borrow();
        assert_eq!(
            host.tree.get(id).unwrap().attributes().get("style"),
            Some(&"width: 1px; height: 2px;".into())
        );
    }

    #[rstest]
    #[case::empty(r#"<div style="">"#)]
    #[case::blank(r#"<div style="  ">"#)]
    #[case::valueless("<div style>")]
    fn an_empty_parsed_style_attribute_sets_no_style_props(#[case] markup: &str) {
        let (engine, host) = engine();

        let id = parse(&engine, markup);

        assert!(host.borrow().tree.get(id).unwrap().style_props().is_empty());
    }

    #[rstest]
    #[case::empty(r#"<div style="">"#)]
    #[case::blank(r#"<div style="  ">"#)]
    #[case::valueless("<div style>")]
    fn an_empty_parsed_style_attribute_warns_nothing(#[case] markup: &str) {
        let (engine, host) = engine();
        let lines = collect_warnings(&host);

        parse(&engine, markup);

        assert!(lines.borrow().is_empty());
    }
}
