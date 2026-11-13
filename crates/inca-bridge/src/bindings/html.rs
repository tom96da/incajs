// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The natives behind `innerHTML`, `outerHTML` and `insertAdjacentHTML`. A write parses the markup
//! in the tree, puts the nodes in place and returns them as a `Parsed` list for JS to build its
//! handles from: `[position, entries]`, where `position` is the index among the children of the
//! container where the first inserted node sits and each entry is
//! `[id, tag, namespace, parentId, data]`. An entry whose `parentId` is no id of the list is a
//! root. `namespace` is the namespace of the node and `null` for a `#text`, `#comment` or
//! `#fragment` node. `data` is the text of a `#text` or `#comment` node and `null` for any other
//! node. The tests of these natives sit in the same file.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Array, Ctx, Exception, IntoJs, Result as JsResult, Value};

use inca_node::{AttributeValue, HtmlScope, NodeId, ParsedHtml, TreeError, VirtualTree};

use super::Natives;
use super::attributes::print_style_warning;
use super::convert::{JsNodeId, existing_node, or_null, throw_tree_error};
use super::host::Host;
use crate::event::array_of;

pub(super) fn install_html(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("innerHtml", inner_html)?;
    n.set("outerHtml", outer_html)?;
    n.set("setInnerHtml", set_inner_html)?;
    n.set("setOuterHtml", set_outer_html)?;
    n.set("insertAdjacentHtml", insert_adjacent_html)
}

/// Where `insertAdjacentHtml` puts the nodes, relative to the node it is called on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Position {
    BeforeBegin,
    AfterBegin,
    BeforeEnd,
    AfterEnd,
}

impl Position {
    /// Reads `text` ignoring ASCII case. Any other string throws a `SyntaxError`.
    fn parse(ctx: &Ctx<'_>, text: &str) -> JsResult<Self> {
        match text.to_ascii_lowercase().as_str() {
            "beforebegin" => Ok(Self::BeforeBegin),
            "afterbegin" => Ok(Self::AfterBegin),
            "beforeend" => Ok(Self::BeforeEnd),
            "afterend" => Ok(Self::AfterEnd),
            _ => Err(Exception::throw_dom(
                ctx,
                "SyntaxError",
                &format!(
                    "unknown position: {text}. use beforebegin, afterbegin, beforeend or afterend"
                ),
            )),
        }
    }
}

fn serialize(
    ctx: &Ctx<'_>,
    host: &Rc<RefCell<Host>>,
    node_id: NodeId,
    scope: HtmlScope,
) -> JsResult<String> {
    host.borrow()
        .tree
        .serialize_html(node_id, scope)
        .map_err(|err| throw_tree_error(ctx, err))
}

fn inner_html<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<String> {
    move |ctx, JsNodeId(node_id)| serialize(&ctx, &host, node_id, HtmlScope::Inner)
}

fn outer_html<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<String> {
    move |ctx, JsNodeId(node_id)| serialize(&ctx, &host, node_id, HtmlScope::Outer)
}

/// Replaces all children of `node_id`, or of its content for a template, with the nodes `markup`
/// parses to in the context of the node.
fn set_inner_html<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<Array<'js>> {
    move |ctx, JsNodeId(node_id), markup| {
        let (container, parsed) = {
            let mut host = host.borrow_mut();
            existing_node(&ctx, &host.tree, node_id)?;
            let container = host.tree.template_content(node_id).unwrap_or(node_id);
            let parsed = host
                .tree
                .parse_html(Some(node_id), &markup)
                .map_err(|err| throw_tree_error(&ctx, err))?;
            (container, parsed)
        };
        let old = children_of(&host.borrow().tree, container);
        lose_focus_in(&ctx, &host, &old);
        let mut host = host.borrow_mut();
        let old = children_of(&host.tree, container);
        for child in old {
            let left = host.leaving(&[child]);
            host.tree
                .remove_child(container, child)
                .map_err(|err| throw_tree_error(&ctx, err))?;
            host.settle(&left);
        }
        let position = insert_parsed(&ctx, &mut host, &parsed, container, None)?;
        parsed_js(&ctx, position, &parsed)
    }
}

/// Replaces `node_id` with the nodes `markup` parses to in the context of its parent. A node
/// without a parent changes nothing and gives `null`. A node whose parent is the document throws.
fn set_outer_html<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id), markup| {
        let parsed = {
            let mut host = host.borrow_mut();
            let Some(parent) = existing_node(&ctx, &host.tree, node_id)?.parent() else {
                return Ok(Value::new_null(ctx.clone()));
            };
            if host.tree.document() == Some(parent) {
                return Err(throw_tree_error(&ctx, TreeError::DocumentParent(node_id)));
            }
            host.tree
                .parse_html(Some(parent), &markup)
                .map_err(|err| throw_tree_error(&ctx, err))?
        };
        lose_focus_in(&ctx, &host, &[node_id]);
        let mut host = host.borrow_mut();
        let Some(parent) = host
            .tree
            .get(node_id)
            .and_then(inca_node::VirtualNode::parent)
        else {
            return Ok(Value::new_null(ctx.clone()));
        };
        let mut left = host.leaving(&[node_id]);
        left.extend(host.leaving(&host.tree.nodes_to_move(parsed.fragment)));
        let position = host
            .tree
            .replace_child(parent, parsed.fragment, node_id)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        host.settle(&left);
        finish(&mut host, &parsed);
        Ok(parsed_js(&ctx, position, &parsed)?.into_value())
    }
}

/// Parses `markup` and inserts the nodes at `position` relative to `node_id`. Nothing leaves the
/// tree.
fn insert_adjacent_html<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String, String) -> JsResult<Array<'js>> {
    move |ctx, JsNodeId(node_id), position, markup| {
        let position = Position::parse(&ctx, &position)?;
        let mut host = host.borrow_mut();
        let node = existing_node(&ctx, &host.tree, node_id)?;
        let container = match position {
            Position::BeforeBegin | Position::AfterEnd => node
                .parent()
                .ok_or_else(|| throw_tree_error(&ctx, TreeError::NoParent(node_id)))?,
            Position::AfterBegin | Position::BeforeEnd => node_id,
        };
        if matches!(position, Position::BeforeBegin | Position::AfterEnd)
            && host.tree.document() == Some(container)
        {
            return Err(throw_tree_error(&ctx, TreeError::DocumentParent(node_id)));
        }
        let context = match host.tree.get(container) {
            Some(node) if node.is_html("html") => None,
            _ => Some(container),
        };
        let parsed = host
            .tree
            .parse_html(context, &markup)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        let reference = match position {
            Position::BeforeBegin => Some(node_id),
            Position::AfterBegin => host.tree.first_child(node_id),
            Position::BeforeEnd => None,
            Position::AfterEnd => host.tree.next_sibling(node_id),
        };
        let position = insert_parsed(&ctx, &mut host, &parsed, container, reference)?;
        parsed_js(&ctx, position, &parsed)
    }
}

/// The children of `id`, or none when the node is gone.
fn children_of(tree: &VirtualTree, id: NodeId) -> Vec<NodeId> {
    tree.get(id)
        .map_or_else(Vec::new, |node| node.children().to_vec())
}

/// Fires `blur` and `focusout` at the focused node when it sits in a subtree of `roots`. A
/// listener may change the tree, so the caller resolves its nodes again afterwards.
fn lose_focus_in(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>, roots: &[NodeId]) {
    let focused = {
        let host = host.borrow();
        host.focus.visual_focus(&host.tree).filter(|&focused| {
            host.tree
                .ancestors(focused)
                .any(|ancestor| roots.contains(&ancestor))
        })
    };
    if let Some(focused) = focused {
        crate::event::lose_focus(ctx, host, focused);
    }
}

/// Moves the nodes of the parse result into `container` before `reference`, or last without one.
/// Returns the position of the first inserted node.
fn insert_parsed(
    ctx: &Ctx<'_>,
    host: &mut Host,
    parsed: &ParsedHtml,
    container: NodeId,
    reference: Option<NodeId>,
) -> JsResult<usize> {
    let left = host.leaving(&host.tree.nodes_to_move(parsed.fragment));
    let position = host
        .tree
        .insert_before(container, parsed.fragment, reference)
        .map_err(|err| throw_tree_error(ctx, err))?;
    host.settle(&left);
    finish(host, parsed);
    Ok(position)
}

/// The steps that follow a parse once its nodes are in place: prints the style warnings, gives an
/// `input` the typed text of its `value` attribute and frees the result fragment, which is empty.
fn finish(host: &mut Host, parsed: &ParsedHtml) {
    for (node_id, line) in &parsed.warnings {
        print_style_warning(host, *node_id, line);
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

/// The `Parsed` value of a parse result put at `position`.
fn parsed_js<'js>(ctx: &Ctx<'js>, position: usize, parsed: &ParsedHtml) -> JsResult<Array<'js>> {
    let mut entries = Vec::with_capacity(parsed.nodes.len());
    for node in &parsed.nodes {
        let row = [
            node.id.into_js(ctx)?,
            node.tag.as_str().into_js(ctx)?,
            or_null(ctx, node.namespace.as_deref())?,
            node.parent.unwrap_or(parsed.fragment).into_js(ctx)?,
            or_null(ctx, node.data.as_deref())?,
        ];
        entries.push(array_of(ctx, row)?.into_value());
    }
    array_of(
        ctx,
        [position.into_js(ctx)?, array_of(ctx, entries)?.into_value()],
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::cell::RefCell;
    use std::rc::Rc;

    use inca_jsenv::Engine;
    use rstest::rstest;

    use crate::Host;
    use crate::test_support::{HTML, LISTEN_JS, MATHML, SVG, engine_with_bindings, error_name};

    /// `host` is a `div` under the root. `rows` maps a `Parsed` result to
    /// `[tag, index of the parent entry or -1, namespace, data]`.
    const SETUP: &str = r"
        const n = __inca_native__;
        const host = n.createNode('div');
        n.appendChild(n.rootNodeId(), host);
        const rows = (parsed) => parsed[1].map((e) => [
            e[1],
            parsed[1].findIndex((x) => x[0] === e[3]),
            e[2],
            e[4],
        ]);
        const html = (id) => n.innerHtml(id);
        ";

    fn engine() -> (Engine, Rc<RefCell<Host>>) {
        let (engine, host) = engine_with_bindings();
        engine.eval::<()>(LISTEN_JS).unwrap();
        engine.eval::<()>(SETUP).unwrap();
        (engine, host)
    }

    fn eval(engine: &Engine, js: &str) -> String {
        engine
            .eval(&format!("JSON.stringify((() => {{ {js} }})())"))
            .unwrap()
    }

    #[test]
    fn set_inner_html_returns_pre_order_entries_with_parent_ids_namespaces_and_data() {
        let (engine, _host) = engine();

        let got = eval(
            &engine,
            r"
            const markup = '<p>a</p>b<!--c--><svg><foreignObject></foreignObject></svg><math><mi>x</mi></math>';
            return rows(n.setInnerHtml(host, markup));
            ",
        );

        assert_eq!(
            got,
            format!(
                r##"[["p",-1,"{HTML}",null],["#text",0,null,"a"],["#text",-1,null,"b"],["#comment",-1,null,"c"],["svg",-1,"{SVG}",null],["foreignObject",4,"{SVG}",null],["math",-1,"{MATHML}",null],["mi",6,"{MATHML}",null],["#text",7,null,"x"]]"##
            )
        );
    }

    #[test]
    fn a_parsed_text_node_has_a_null_namespace() {
        let (engine, _host) = engine();

        let strict: bool = engine
            .eval("n.setInnerHtml(host, 'a')[1][0][2] === null;")
            .unwrap();

        assert!(strict);
    }

    #[rstest]
    #[case::html("<input value=a>", 0, true)]
    #[case::svg("<svg><input value=a></svg>", 1, false)]
    fn a_parsed_input_takes_the_value_attribute_as_typed_text_in_html_only(
        #[case] markup: &str,
        #[case] index: usize,
        #[case] typed: bool,
    ) {
        let (engine, host) = engine();

        let id: u32 = engine
            .eval(&format!("n.setInnerHtml(host, '{markup}')[1][{index}][0];"))
            .unwrap();

        assert_eq!(host.borrow().inputs.borrow().contains_key(&id), typed);
    }

    #[test]
    fn set_outer_html_parses_in_the_namespace_of_the_parent() {
        let (engine, _host) = engine();

        let got = eval(
            &engine,
            r"
            const svg = n.setInnerHtml(host, '<svg><foreignObject></foreignObject></svg>')[1][0][0];
            const foreign = n.childNodeIds(svg)[0];
            return rows(n.setOuterHtml(foreign, '<title>'));
            ",
        );

        assert_eq!(got, format!(r#"[["title",-1,"{SVG}",null]]"#));
    }

    #[test]
    fn set_inner_html_replaces_all_old_children_and_returns_position_zero() {
        let (engine, _host) = engine();

        let got = eval(
            &engine,
            r"
            n.setInnerHtml(host, '<b></b><i></i>');
            const parsed = n.setInnerHtml(host, '<u></u>');
            return [parsed[0], html(host)];
            ",
        );

        assert_eq!(got, r#"[0,"<u></u>"]"#);
    }

    #[test]
    fn an_old_child_keeps_its_listener_after_set_inner_html() {
        let (engine, _host) = engine();

        let got = eval(
            &engine,
            r"
            const old = n.createNode('i');
            n.appendChild(host, old);
            globalThis.count = 0;
            __inca_test_listen__(old, 'ping', () => {
                count += 1;
            });
            n.setInnerHtml(host, '');
            n.dispatchEvent(old, new Event('ping'));
            return count;
            ",
        );

        assert_eq!(got, "1");
    }

    #[test]
    fn set_inner_html_on_a_template_fills_its_content_and_no_own_child() {
        let (engine, _host) = engine();

        let got = eval(
            &engine,
            r"
            const t = n.createNode('template');
            n.appendChild(host, t);
            n.setInnerHtml(t, '<b>x</b>');
            const content = n.templateContent(t);
            return [n.childNodeIds(t).length, n.childNodeIds(content).length, html(t)];
            ",
        );

        assert_eq!(got, r#"[0,1,"<b>x</b>"]"#);
    }

    #[test]
    fn insert_adjacent_before_end_on_a_template_adds_an_own_child() {
        let (engine, _host) = engine();

        let got = eval(
            &engine,
            r"
            const t = n.createNode('template');
            n.appendChild(host, t);
            n.setInnerHtml(t, '<b>x</b>');
            n.insertAdjacentHtml(t, 'beforeend', '<u>y</u>');
            return [n.childNodeIds(t).length, n.childNodeIds(n.templateContent(t)).length];
            ",
        );

        assert_eq!(got, "[1,1]");
    }

    #[test]
    fn set_outer_html_replaces_the_node_and_returns_its_position() {
        let (engine, _host) = engine();

        let got = eval(
            &engine,
            r"
            n.setInnerHtml(host, '<a></a><b></b>');
            const b = n.childNodeIds(host)[1];
            const parsed = n.setOuterHtml(b, '<i></i><u></u>');
            return [parsed[0], html(host)];
            ",
        );

        assert_eq!(got, r#"[1,"<a></a><i></i><u></u>"]"#);
    }

    #[test]
    fn set_outer_html_on_a_parentless_node_returns_null_and_changes_nothing() {
        let (engine, _host) = engine();

        let got = eval(
            &engine,
            r"
            const loose = n.createNode('p');
            return [n.setOuterHtml(loose, '<i></i>'), n.outerHtml(loose)];
            ",
        );

        assert_eq!(got, r#"[null,"<p></p>"]"#);
    }

    /// The error name of `call` and how many ids the next `createNode` skips after it.
    fn name_and_skipped_ids(engine: &Engine, call: &str) -> String {
        eval(
            engine,
            &format!(
                r"
                const before = n.createNode('p');
                let name = 'none';
                try {{
                    {call};
                }} catch (e) {{
                    name = e.name;
                }}
                return [name, n.createNode('p') - before - 1];
                "
            ),
        )
    }

    #[test]
    fn set_outer_html_on_the_element_of_the_document_throws_before_parsing() {
        let (engine, _host) = engine();

        assert_eq!(
            name_and_skipped_ids(&engine, "n.setOuterHtml(n.rootNodeId(), '<i></i>')"),
            r#"["NoModificationAllowedError",0]"#
        );
    }

    const PLACED: &str = r"
        n.setInnerHtml(host, '<p></p><m><i></i></m><q></q>');
        const m = n.childNodeIds(host)[1];
        const parsed = n.insertAdjacentHtml(m, POSITION, '<x></x>');
        return [parsed[0], html(host)];
        ";

    #[rstest]
    #[case::before_begin("beforebegin", r#"[1,"<p></p><x></x><m><i></i></m><q></q>"]"#)]
    #[case::after_begin("afterbegin", r#"[0,"<p></p><m><x></x><i></i></m><q></q>"]"#)]
    #[case::before_end("beforeend", r#"[1,"<p></p><m><i></i><x></x></m><q></q>"]"#)]
    #[case::after_end("afterend", r#"[2,"<p></p><m><i></i></m><x></x><q></q>"]"#)]
    fn insert_adjacent_html_places_the_nodes_at_the_position(
        #[case] position: &str,
        #[case] expected: &str,
    ) {
        let (engine, _host) = engine();

        let got = eval(
            &engine,
            &PLACED.replace("POSITION", &format!("'{position}'")),
        );

        assert_eq!(got, expected);
    }

    #[rstest]
    #[case::any_case("BeforeEnd", "none")]
    #[case::unknown("inside", "SyntaxError")]
    fn insert_adjacent_html_reads_a_position_ignoring_case(
        #[case] position: &str,
        #[case] expected: &str,
    ) {
        let (engine, _host) = engine();

        assert_eq!(
            error_name(
                &engine,
                &format!("n.insertAdjacentHtml(host, '{position}', '<i></i>')")
            ),
            expected
        );
    }

    #[test]
    fn a_bad_position_on_a_parentless_node_throws_syntax_error() {
        let (engine, _host) = engine();

        assert_eq!(
            error_name(
                &engine,
                "n.insertAdjacentHtml(n.createNode('p'), 'inside', 'x')"
            ),
            "SyntaxError"
        );
    }

    #[rstest]
    #[case::before_begin("beforebegin")]
    #[case::after_end("afterend")]
    fn a_sibling_position_on_a_parentless_node_throws_no_modification_allowed(
        #[case] position: &str,
    ) {
        let (engine, _host) = engine();

        assert_eq!(
            error_name(
                &engine,
                &format!("n.insertAdjacentHtml(n.createNode('p'), '{position}', 'x')")
            ),
            "NoModificationAllowedError"
        );
    }

    #[rstest]
    #[case::after_end("afterend")]
    #[case::before_begin("beforebegin")]
    fn a_sibling_position_on_the_attached_root_throws_before_parsing(#[case] position: &str) {
        let (engine, _host) = engine();

        assert_eq!(
            name_and_skipped_ids(
                &engine,
                &format!("n.insertAdjacentHtml(n.rootNodeId(), '{position}', 'x')")
            ),
            r#"["NoModificationAllowedError",0]"#
        );
    }

    #[rstest]
    #[case::before_begin_of_a_child("beforebegin", "child", "x<body></body>")]
    #[case::after_begin_of_html("afterbegin", "root", "x<body></body>")]
    fn an_html_context_parses_in_the_body_context(
        #[case] position: &str,
        #[case] target: &str,
        #[case] expected: &str,
    ) {
        let (engine, _host) = engine();

        let got = eval(
            &engine,
            &format!(
                r"
                const root = n.createNode('html');
                const child = n.createNode('body');
                n.appendChild(root, child);
                n.insertAdjacentHtml({target}, '{position}', '<td>x</td>');
                return html(root);
                "
            ),
        );

        assert_eq!(got, format!("\"{expected}\""));
    }

    const FOCUSED: &str = r"
        const btn = n.createNode('button');
        n.appendChild(host, btn);
        n.focusNode(btn);
        globalThis.out = [];
        for (const type of ['blur', 'focusout']) {
            __inca_test_listen__(btn, type, (e) => {
                out.push(e.type);
            });
        }
        ";

    #[test]
    fn a_refused_set_outer_html_fires_no_focus_event() {
        let (engine, _host) = engine();
        engine.eval::<()>(FOCUSED).unwrap();

        let _ = error_name(&engine, "n.setOuterHtml(n.rootNodeId(), '<i></i>')");

        assert_eq!(eval(&engine, "return out;"), "[]");
    }

    #[test]
    fn an_unknown_position_leaves_no_orphan() {
        let (engine, host) = engine();

        let _ = error_name(&engine, "n.insertAdjacentHtml(host, 'inside', '<i></i>')");

        assert!(!host.borrow().tree.has_orphans());
    }

    #[test]
    fn nothing_fires_in_an_inactive_window() {
        let (engine, host) = engine();
        engine.eval::<()>(FOCUSED).unwrap();
        host.borrow_mut().focus.set_window_blurred(true);

        let got = eval(&engine, "n.setInnerHtml(host, ''); return out;");

        assert_eq!(got, "[]");
    }

    #[rstest]
    #[case::inner("n.setInnerHtml(host, '')")]
    #[case::outer("n.setOuterHtml(btn, '')")]
    fn a_replacement_fires_the_focus_events_and_drops_the_focus(#[case] call: &str) {
        let (engine, _host) = engine();
        engine.eval::<()>(FOCUSED).unwrap();

        let got = eval(
            &engine,
            &format!(
                r"
                {call};
                return [out, n.focusedNode()];
                "
            ),
        );

        assert_eq!(got, r#"[["blur","focusout"],null]"#);
    }

    #[test]
    fn nothing_fires_when_the_focused_node_stays() {
        let (engine, _host) = engine();

        let got = eval(
            &engine,
            &format!(
                r"
                {FOCUSED}
                const other = n.createNode('div');
                n.appendChild(host, other);
                n.setInnerHtml(other, '<i></i>');
                n.insertAdjacentHtml(host, 'beforeend', '<u></u>');
                return [out, n.focusedNode() === btn];
                "
            ),
        );

        assert_eq!(got, "[[],true]");
    }

    #[test]
    fn style_warnings_print_once_for_each_node_and_line() {
        let (engine, host) = engine();
        let lines = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&lines);
        host.borrow_mut().warn = Rc::new(move |line| sink.borrow_mut().push(line.to_owned()));

        engine
            .eval::<()>(
                "n.setInnerHtml(host, '<p style=\"bogus:1;bogus:2\"><i style=\"bogus:1\"></i></p>');",
            )
            .unwrap();

        let lines = lines.borrow();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].contains("(p)"), "{lines:?}");
        assert!(lines[1].contains("(i)"), "{lines:?}");
    }

    #[test]
    fn an_unknown_node_throws_type_error() {
        let (engine, _host) = engine();

        assert_eq!(error_name(&engine, "n.setInnerHtml(999, '')"), "TypeError");
    }

    #[rstest]
    #[case::inner("html(host)", "<b>x</b>")]
    #[case::outer("n.outerHtml(host)", "<div><b>x</b></div>")]
    fn the_html_getters_read_the_markup_of_a_node(#[case] read: &str, #[case] expected: &str) {
        let (engine, _host) = engine();

        let got = eval(
            &engine,
            &format!(
                r"
                n.setInnerHtml(host, '<b>x</b>');
                return {read};
                "
            ),
        );

        assert_eq!(got, format!("\"{expected}\""));
    }
}
