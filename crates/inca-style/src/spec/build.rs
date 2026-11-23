// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The element spec of a node and its subtree: the kind of each element, its style, its
//! inertness and the hiding by the `hidden` attribute.

use super::parse::style_spec_from_props;
use super::{DisplaySpec, StyleSpec};
use crate::StyleCache;
use inca_node::input_text::sanitized;
use inca_node::tree::{AttributeValue, NodeId, NodeKind, StyleMap, VirtualNode, VirtualTree};

/// What kind of element a [`VirtualNode`] maps to.
///
/// Three kinds exist, chosen by the node kind and the `"input"` tag name.
///
/// Text is always its own node with a stable id, never a bare string child
/// of a container. Every rendered text run is addressable by [`NodeId`].
#[derive(Debug, Clone, PartialEq)]
pub enum ElementTag {
    /// Any element tag name other than `"input"`: a generic styled box.
    Container,
    /// A `"#text"` tag: its data. Its children are not rendered.
    Text(String),
    /// An `"input"` tag: one line of text held per node, drawn
    /// over the box. `placeholder` is its `placeholder` attribute without CR
    /// and LF ([`inca_node::input_text::sanitized`]; missing or non-string → empty).
    /// `editable` is false with a `readonly` or `disabled` attribute. Its
    /// children are not rendered.
    Input { placeholder: String, editable: bool },
}

/// A recursive description of one [`VirtualNode`] and its subtree.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementSpec {
    pub id: NodeId,
    pub tag: ElementTag,
    pub style: StyleSpec,
    /// The node or an ancestor has the `inert` attribute. The node reports
    /// no mouse target and no hover.
    pub inert: bool,
    pub children: Vec<ElementSpec>,
}

/// Builds an [`ElementSpec`] for `root` and its whole subtree. `None` if `root` doesn't resolve
/// (the convention of [`VirtualTree::get`]) or is a comment, a `<style>` element or a template.
/// It skips a child id that doesn't resolve.
#[must_use]
pub fn build_spec(tree: &VirtualTree, cache: &StyleCache, root: NodeId) -> Option<ElementSpec> {
    let mut inert = false;
    let mut cursor = tree.get(root)?.parent();
    while let Some(ancestor) = cursor.and_then(|id| tree.get(id)) {
        inert |= ancestor.attributes().contains_key("inert");
        cursor = ancestor.parent();
    }
    build_spec_inner(tree, cache, root, inert)
}

/// Whether the `hidden` attribute hides `node`: it is present in the Hidden
/// state (any value except `until-found`) and `computed` sets no `display`.
fn hidden_with(node: &VirtualNode, computed: &StyleMap) -> bool {
    let until_found = matches!(
        node.attributes().get("hidden"),
        Some(AttributeValue::String(value)) if value.eq_ignore_ascii_case("until-found")
    );
    let display_set = computed
        .get("display")
        .is_some_and(|value| value.as_str() != Some(""));
    node.attributes().contains_key("hidden") && !until_found && !display_set
}

/// Whether the `hidden` attribute hides `id`: it is present in the Hidden
/// state (any value except `until-found`) and the style of the node sets no
/// `display`. `false` for an unknown id.
#[must_use]
pub fn hidden_by_attribute(tree: &VirtualTree, cache: &StyleCache, id: NodeId) -> bool {
    tree.get(id)
        .is_some_and(|node| hidden_with(node, &cache.computed_style(tree, id)))
}

fn build_spec_inner(
    tree: &VirtualTree,
    cache: &StyleCache,
    root: NodeId,
    inert_above: bool,
) -> Option<ElementSpec> {
    let node = tree.get(root)?;
    // A `<style>` element holds rules, a template holds markup and a comment holds a note. None
    // takes part in the layout.
    if node.is_style() || node.is_template() || node.kind() == NodeKind::Comment {
        return None;
    }
    let inert = inert_above || node.attributes().contains_key("inert");

    let tag = match node.kind() {
        NodeKind::Text => ElementTag::Text(node.data().to_owned()),
        _ if node.is_html("input") => {
            let attributes = node.attributes();
            ElementTag::Input {
                placeholder: attributes
                    .get("placeholder")
                    .and_then(AttributeValue::as_str)
                    // `shape_line` takes one line.
                    .map(sanitized)
                    .unwrap_or_default(),
                editable: !attributes.contains_key("readonly")
                    && !attributes.contains_key("disabled"),
            }
        }
        _ => ElementTag::Container,
    };

    let computed = cache.computed_style(tree, root);
    let mut style = style_spec_from_props(&computed);
    if hidden_with(node, &computed) {
        style.display = Some(DisplaySpec::None);
    }
    let children = if matches!(tag, ElementTag::Text(_) | ElementTag::Input { .. }) {
        Vec::new()
    } else {
        node.children()
            .iter()
            .filter_map(|&child_id| build_spec_inner(tree, cache, child_id, inert))
            .collect()
    };

    Some(ElementSpec {
        id: root,
        tag,
        style,
        inert,
        children,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::tests::{Fixture, HTML, SVG, classed};
    use crate::{remove_style, set_style};

    #[test]
    fn unknown_root_returns_none() {
        let tree = VirtualTree::new();
        assert!(build_spec(&tree, &StyleCache::new(), 42).is_none());
    }

    #[test]
    fn non_text_tag_is_a_container() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();

        let spec = build_spec(&tree, &StyleCache::new(), id).unwrap();
        assert_eq!(spec.tag, ElementTag::Container);
    }

    #[rstest]
    #[case::plain(vec![], "")]
    #[case::placeholder(vec![AttributeValue::from("hi")], "hi")]
    #[case::placeholder_with_newline(vec![AttributeValue::from("a\nb")], "ab")]
    #[case::number_placeholder(vec![AttributeValue::from(3.0)], "")]
    fn an_input_spec_reads_its_placeholder(
        #[case] placeholder: Vec<AttributeValue>,
        #[case] want: &str,
    ) {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("input").unwrap();
        for value in placeholder {
            tree.set_attribute(id, "placeholder", value).unwrap();
        }
        let want = ElementTag::Input {
            placeholder: want.to_owned(),
            editable: true,
        };
        assert_eq!(build_spec(&tree, &StyleCache::new(), id).unwrap().tag, want);
    }

    #[rstest]
    #[case::plain(&[], true)]
    #[case::readonly(&["readonly"], false)]
    #[case::disabled(&["disabled"], false)]
    fn an_input_spec_is_editable_without_readonly_or_disabled(
        #[case] flags: &[&str],
        #[case] want: bool,
    ) {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("input").unwrap();
        for flag in flags {
            tree.set_attribute(id, *flag, true).unwrap();
        }
        let want = ElementTag::Input {
            placeholder: String::new(),
            editable: want,
        };
        assert_eq!(build_spec(&tree, &StyleCache::new(), id).unwrap().tag, want);
    }

    #[test]
    fn an_input_spec_has_no_children() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("input").unwrap();
        let child = tree.create_node("div").unwrap();
        tree.append_child(id, child).unwrap();
        assert!(
            build_spec(&tree, &StyleCache::new(), id)
                .unwrap()
                .children
                .is_empty()
        );
    }

    #[test]
    fn the_hidden_attribute_makes_the_spec_display_none_unless_display_is_set() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        let display = |tree: &VirtualTree| {
            build_spec(tree, &StyleCache::new(), id)
                .unwrap()
                .style
                .display
        };
        assert_eq!(display(&tree), None);
        for (value, want) in [
            ("", Some(DisplaySpec::None)),
            ("hidden", Some(DisplaySpec::None)),
            ("until-found", None),
            ("UNTIL-FOUND", None),
        ] {
            tree.set_attribute(id, "hidden", value).unwrap();
            assert_eq!(display(&tree), want, "{value:?}");
        }
        tree.set_attribute(id, "hidden", true).unwrap();
        assert_eq!(display(&tree), Some(DisplaySpec::None));
        set_style(&mut tree, id, "display", "flex").unwrap();
        assert_eq!(display(&tree), Some(DisplaySpec::Flex));
        remove_style(&mut tree, id, "display").unwrap();
        tree.remove_attribute(id, "hidden").unwrap();
        assert_eq!(display(&tree), None);
    }

    #[test]
    fn an_inert_node_and_its_subtree_are_inert() {
        let mut tree = VirtualTree::new();
        let [outer, inert, child, sibling] = ["div"; 4].map(|tag| tree.create_node(tag).unwrap());
        tree.append_child(outer, inert).unwrap();
        tree.append_child(inert, child).unwrap();
        tree.append_child(outer, sibling).unwrap();
        tree.set_attribute(inert, "inert", true).unwrap();
        let inerts = |root| {
            let spec = build_spec(&tree, &StyleCache::new(), root).unwrap();
            let mut seen = vec![(spec.id, spec.inert)];
            let mut stack = spec.children;
            while let Some(spec) = stack.pop() {
                seen.push((spec.id, spec.inert));
                stack.extend(spec.children);
            }
            seen.sort_unstable();
            seen
        };
        assert_eq!(
            inerts(outer),
            [
                (outer, false),
                (inert, true),
                (child, true),
                (sibling, false)
            ]
        );
        // A subtree built from the child alone still sees the inert ancestor.
        assert_eq!(inerts(child), [(child, true)]);
    }

    #[test]
    fn the_hidden_attribute_hides_a_node_with_an_empty_display_and_no_sheet() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        set_style(&mut tree, node, "display", "").unwrap();
        tree.set_attribute(node, "hidden", true).unwrap();

        assert!(hidden_by_attribute(&tree, &StyleCache::new(), node));
    }

    #[test]
    fn the_hidden_attribute_hides_a_node_with_an_empty_display_and_no_matching_rule() {
        let mut f = Fixture::new();
        f.sheet(".b { width: 1px }");
        let node = classed(&mut f, "a");
        set_style(&mut f.tree, node, "display", "").unwrap();
        f.attr(node, "hidden", true);

        assert!(hidden_by_attribute(&f.tree, &f.cache, node));
    }

    #[test]
    fn a_style_element_builds_no_element() {
        let mut f = Fixture::new();
        let (style, _) = f.sheet(".a { width: 1px }");
        let other = classed(&mut f, "a");

        let spec = build_spec(&f.tree, &f.cache, f.root).unwrap();

        assert!(build_spec(&f.tree, &f.cache, style).is_none());
        assert_eq!(
            spec.children
                .iter()
                .map(|child| child.id)
                .collect::<Vec<_>>(),
            [other]
        );
    }

    #[test]
    fn a_comment_builds_no_element() {
        let mut f = Fixture::new();
        let comment = f.tree.create_node("#comment").unwrap();
        f.tree.append_child(f.root, comment).unwrap();

        assert!(build_spec(&f.tree, &f.cache, comment).is_none());
    }

    #[test]
    fn a_comment_leaves_no_child_in_its_parent() {
        let mut f = Fixture::new();
        let comment = f.tree.create_node("#comment").unwrap();
        f.tree.append_child(f.root, comment).unwrap();
        let other = classed(&mut f, "a");

        let spec = build_spec(&f.tree, &f.cache, f.root).unwrap();

        assert_eq!(
            spec.children
                .iter()
                .map(|child| child.id)
                .collect::<Vec<_>>(),
            [other]
        );
    }

    #[test]
    fn a_rule_display_stops_the_hidden_attribute_from_hiding() {
        let mut f = Fixture::new();
        f.sheet(".s { display: flex }");
        let node = classed(&mut f, "s");
        f.attr(node, "hidden", true);

        assert!(!hidden_by_attribute(&f.tree, &f.cache, node));
        assert_eq!(
            build_spec(&f.tree, &f.cache, node).unwrap().style.display,
            Some(DisplaySpec::Flex)
        );
    }

    #[test]
    fn a_text_named_tag_is_a_generic_box() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("text").unwrap();
        tree.set_attribute(id, "value", "hello").unwrap();

        let spec = build_spec(&tree, &StyleCache::new(), id).unwrap();
        assert_eq!(spec.tag, ElementTag::Container);
    }

    #[test]
    fn a_text_node_renders_its_data() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("#text").unwrap();
        tree.set_data(id, "hello").unwrap();

        let spec = build_spec(&tree, &StyleCache::new(), id).unwrap();
        assert_eq!(spec.tag, ElementTag::Text("hello".into()));
    }

    #[test]
    fn a_text_node_without_data_renders_empty() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("#text").unwrap();

        let spec = build_spec(&tree, &StyleCache::new(), id).unwrap();
        assert_eq!(spec.tag, ElementTag::Text(String::new()));
    }

    #[test]
    fn children_are_built_in_append_order() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div").unwrap();
        let a = tree.create_node("div").unwrap();
        let b = tree.create_node("div").unwrap();
        tree.append_child(parent, a).unwrap();
        tree.append_child(parent, b).unwrap();

        let spec = build_spec(&tree, &StyleCache::new(), parent).unwrap();
        let child_ids: Vec<NodeId> = spec.children.iter().map(|c| c.id).collect();
        assert_eq!(child_ids, &[a, b]);
    }

    #[test]
    fn a_template_without_children_has_no_spec() {
        let mut tree = VirtualTree::new();
        let template = tree.create_node("template").unwrap();

        assert!(build_spec(&tree, &StyleCache::new(), template).is_none());
    }

    #[test]
    fn a_template_with_a_child_has_no_spec() {
        let mut tree = VirtualTree::new();
        let template = tree.create_node("template").unwrap();
        let child = tree.create_node("div").unwrap();
        tree.append_child(template, child).unwrap();

        assert!(build_spec(&tree, &StyleCache::new(), template).is_none());
    }

    #[test]
    fn an_svg_template_is_laid_out() {
        let mut tree = VirtualTree::new();
        let template = tree.create_element_ns(Some(SVG), None, "template").unwrap();

        assert!(build_spec(&tree, &StyleCache::new(), template).is_some());
    }

    #[test]
    fn an_svg_input_is_a_container() {
        let mut tree = VirtualTree::new();
        let input = tree.create_element_ns(Some(SVG), None, "input").unwrap();
        let child = tree.create_node("div").unwrap();
        tree.append_child(input, child).unwrap();

        let spec = build_spec(&tree, &StyleCache::new(), input).unwrap();
        assert!(matches!(spec.tag, ElementTag::Container));
        assert_eq!(spec.children.len(), 1);
    }

    #[rstest]
    #[case::html(HTML)]
    #[case::svg(SVG)]
    fn a_style_element_has_no_spec(#[case] namespace: &str) {
        let mut tree = VirtualTree::new();
        let style = tree
            .create_element_ns(Some(namespace), None, "style")
            .unwrap();

        assert!(build_spec(&tree, &StyleCache::new(), style).is_none());
    }
}
