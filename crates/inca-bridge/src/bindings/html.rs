// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The natives behind `innerHTML`, `outerHTML` and `insertAdjacentHTML`. A write parses the markup
//! in the tree and puts the nodes in place. `setInnerHtml` and `setOuterHtml` return the removed
//! subtrees, and `insertAdjacentHtml` returns nothing. The tests of these natives sit in the same
//! file.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Array, Ctx, Exception, Result as JsResult};

use inca_node::{NodeId, ParsedHtml, TreeError, VirtualNode, VirtualTree};

use super::Natives;
use super::convert::{JsNodeId, existing_node, throw_tree_error};
use super::host::Host;
use super::mutation::removed_subtrees;

mod finish;
use finish::finish;

pub(super) fn install_html(n: &Natives<'_, '_>) -> JsResult<()> {
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

/// Replaces all children of `node_id`, or of its content for a template, with the nodes `markup`
/// parses to in the context of the node. Returns the removed subtrees.
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
        for &child in &old {
            let left = host.leaving(&[child]);
            host.tree
                .remove_child(container, child)
                .map_err(|err| throw_tree_error(&ctx, err))?;
            host.settle(&left);
        }
        insert_parsed(&ctx, &mut host, &parsed, container, None)?;
        removed_subtrees(&ctx, &host.tree, &old)
    }
}

/// Replaces `node_id` with the nodes `markup` parses to in the context of its parent and returns
/// the removed subtrees. A node without a parent changes nothing and gives none. A node whose
/// parent is a document throws.
fn set_outer_html<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<Array<'js>> {
    move |ctx, JsNodeId(node_id), markup| {
        let parsed = {
            let mut host = host.borrow_mut();
            let Some(parent) = existing_node(&ctx, &host.tree, node_id)?.parent() else {
                return Array::new(ctx.clone());
            };
            if host.tree.get(parent).is_some_and(VirtualNode::is_document) {
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
            return Array::new(ctx.clone());
        };
        let mut left = host.leaving(&[node_id]);
        left.extend(host.leaving(&host.tree.nodes_to_move(parsed.fragment)));
        host.tree
            .replace_child(parent, parsed.fragment, node_id)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        host.settle(&left);
        finish(&mut host, &parsed);
        removed_subtrees(&ctx, &host.tree, &[node_id])
    }
}

/// Parses `markup` and inserts the nodes at `position` relative to `node_id`. Nothing leaves the
/// tree.
fn insert_adjacent_html<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String, String) -> JsResult<()> {
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
            && host
                .tree
                .get(container)
                .is_some_and(VirtualNode::is_document)
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
        insert_parsed(&ctx, &mut host, &parsed, container, reference)?;
        Ok(())
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
        host.focus
            .visual_focus(&host.tree, &host.style)
            .filter(|&focused| {
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
fn insert_parsed(
    ctx: &Ctx<'_>,
    host: &mut Host,
    parsed: &ParsedHtml,
    container: NodeId,
    reference: Option<NodeId>,
) -> JsResult<()> {
    let left = host.leaving(&host.tree.nodes_to_move(parsed.fragment));
    host.tree
        .insert_before(container, parsed.fragment, reference)
        .map_err(|err| throw_tree_error(ctx, err))?;
    host.settle(&left);
    finish(host, parsed);
    Ok(())
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

    /// `host` is a `div` under the root. `rows(id)` lists the descendants of `id` in pre-order as
    /// `[tag, index of the parent row or -1, namespace, data]`.
    const SETUP: &str = r"
        const n = __inca_native__;
        const host = n.createNode('div');
        n.appendChild(n.rootNodeId(), host);
        const protos = n.interfacePrototypes();
        const rows = (root) => {
            const out = [];
            const walk = (id, parent) => {
                for (const child of n.childNodeIds(id)) {
                    const handle = n.nodeHandle(child)[0];
                    const proto = Object.getPrototypeOf(handle);
                    let leaf = null;
                    if (proto === protos.Text) {
                        leaf = '#text';
                    } else if (proto === protos.Comment) {
                        leaf = '#comment';
                    }
                    const at = out.length;
                    out.push([
                        leaf ?? handle.localName,
                        parent,
                        leaf ? null : handle.namespaceURI,
                        leaf ? n.textContent(child) : null,
                    ]);
                    walk(child, at);
                }
            };
            walk(root, -1);
            return out;
        };
        const html = (id) => n.nodeHandle(id)[0].innerHTML;
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
    fn set_inner_html_puts_the_parsed_nodes_in_place_with_their_namespaces_and_data() {
        let (engine, _host) = engine();

        let got = eval(
            &engine,
            r"
            const markup = '<p>a</p>b<!--c--><svg><foreignObject></foreignObject></svg><math><mi>x</mi></math>';
            n.setInnerHtml(host, markup);
            return rows(host);
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
    fn set_outer_html_parses_in_the_namespace_of_the_parent() {
        let (engine, _host) = engine();

        let got = eval(
            &engine,
            r"
            n.setInnerHtml(host, '<svg><foreignObject></foreignObject></svg>');
            const svg = n.childNodeIds(host)[0];
            n.setOuterHtml(n.childNodeIds(svg)[0], '<title>');
            return rows(svg);
            ",
        );

        assert_eq!(got, format!(r#"[["title",-1,"{SVG}",null]]"#));
    }

    #[test]
    fn set_inner_html_replaces_all_old_children() {
        let (engine, _host) = engine();

        let got = eval(
            &engine,
            r"
            n.setInnerHtml(host, '<b></b><i></i>');
            n.setInnerHtml(host, '<u></u>');
            return html(host);
            ",
        );

        assert_eq!(got, r#""<u></u>""#);
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
    fn set_outer_html_replaces_the_node_and_returns_its_subtree() {
        let (engine, _host) = engine();

        let got = eval(
            &engine,
            r"
            n.setInnerHtml(host, '<a></a><b><s></s></b>');
            const b = n.childNodeIds(host)[1];
            const s = n.childNodeIds(b)[0];
            const removed = n.setOuterHtml(b, '<i></i><u></u>');
            return [removed.map((ids) => ids.map((id) => [b, s].indexOf(id))), html(host)];
            ",
        );

        assert_eq!(got, r#"[[[0,1]],"<a></a><i></i><u></u>"]"#);
    }

    #[test]
    fn set_inner_html_returns_the_subtree_of_each_old_child() {
        let (engine, _host) = engine();

        let got = eval(
            &engine,
            r"
            n.setInnerHtml(host, '<a><s></s></a><b></b>');
            const [a, b] = n.childNodeIds(host);
            const s = n.childNodeIds(a)[0];
            const removed = n.setInnerHtml(host, '<i></i>');
            return removed.map((ids) => ids.map((id) => [a, s, b].indexOf(id)));
            ",
        );

        assert_eq!(got, "[[0,1],[2]]");
    }

    #[test]
    fn set_inner_html_on_a_template_returns_the_subtrees_of_its_content_children() {
        let (engine, _host) = engine();

        let got = eval(
            &engine,
            r"
            const t = n.createNode('template');
            n.appendChild(host, t);
            n.setInnerHtml(t, '<b>x</b>');
            const b = n.childNodeIds(n.templateContent(t))[0];
            const removed = n.setInnerHtml(t, '');
            return removed.map((ids) => ids.map((id) => id === b ? 'b' : 'text'));
            ",
        );

        assert_eq!(got, r#"[["b","text"]]"#);
    }

    #[test]
    fn set_outer_html_on_a_parentless_node_returns_no_subtree_and_changes_nothing() {
        let (engine, _host) = engine();

        let got = eval(
            &engine,
            r"
            const loose = n.createNode('p');
            return [n.setOuterHtml(loose, '<i></i>'), n.nodeHandle(loose)[0].outerHTML];
            ",
        );

        assert_eq!(got, r#"[[],"<p></p>"]"#);
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

    #[rstest]
    #[case::set_outer_html("n.setOuterHtml(copy, '<i></i>')")]
    #[case::insert_adjacent_html_after_end("n.insertAdjacentHtml(copy, 'afterend', '<i></i>')")]
    fn markup_beside_the_element_of_a_copied_document_throws(#[case] call: &str) {
        let (engine, _host) = engine();

        assert_eq!(
            error_name(
                &engine,
                &format!(
                    r"
                    const copy = n.childNodeIds(n.cloneNode(n.documentNodeId(), true))[0];
                    {call};
                    "
                )
            ),
            "NoModificationAllowedError"
        );
    }

    const PLACED: &str = r"
        n.setInnerHtml(host, '<p></p><m><i></i></m><q></q>');
        const m = n.childNodeIds(host)[1];
        n.insertAdjacentHtml(m, POSITION, '<x></x>');
        return html(host);
        ";

    #[rstest]
    #[case::before_begin("beforebegin", r#""<p></p><x></x><m><i></i></m><q></q>""#)]
    #[case::after_begin("afterbegin", r#""<p></p><m><x></x><i></i></m><q></q>""#)]
    #[case::before_end("beforeend", r#""<p></p><m><i></i><x></x></m><q></q>""#)]
    #[case::after_end("afterend", r#""<p></p><m><i></i></m><x></x><q></q>""#)]
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
    fn insert_adjacent_html_returns_undefined() {
        let (engine, _host) = engine();

        let got: bool = engine
            .eval("n.insertAdjacentHtml(host, 'beforeend', '<i></i>') === undefined")
            .unwrap();

        assert!(got);
    }

    #[test]
    fn an_unknown_node_throws_type_error() {
        let (engine, _host) = engine();

        assert_eq!(error_name(&engine, "n.setInnerHtml(999, '')"), "TypeError");
    }
}
