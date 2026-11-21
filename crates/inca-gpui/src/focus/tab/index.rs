// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The tab index of a node and the nodes whose tab index may have changed.

use super::super::{
    AttributeValue, FocusRegistry, NodeId, StyleCache, VirtualTree, hidden_by_attribute,
    is_disabled_control,
};

impl FocusRegistry {
    /// Whether `node_id` can hold focus once this frame's tab check has run.
    /// A node in `tab_dirty` holds it when it has a tab index, and any other
    /// node holds it when it has a handle.
    pub(in crate::focus) fn focusable_after_frame(
        &self,
        tree: &VirtualTree,
        cache: &StyleCache,
        node_id: NodeId,
    ) -> bool {
        if self.tab_dirty.contains(&node_id) {
            Self::tab_index_of(tree, cache, node_id).is_some()
        } else {
            self.handles.contains_key(&node_id)
        }
    }

    /// Records that `node_id`'s `tabindex` may have changed. The next
    /// [`Self::apply_pending`] re-reads it.
    pub fn mark_tab_dirty(&mut self, node_id: NodeId) {
        self.tab_dirty.insert(node_id);
    }

    /// Takes the nodes marked by [`Self::mark_tab_dirty`] and its subtree form.
    #[cfg(feature = "test-support")]
    pub fn take_tab_dirty(&mut self) -> std::collections::HashSet<NodeId> {
        std::mem::take(&mut self.tab_dirty)
    }

    /// Like [`Self::mark_tab_dirty`] for `node_id` and its descendants that
    /// have, or could take, a handle.
    pub fn mark_subtree_dirty(&mut self, tree: &VirtualTree, node_id: NodeId) {
        let mut stack = vec![node_id];
        while let Some(id) = stack.pop() {
            let Some(node) = tree.get(id) else { continue };
            if self.handles.contains_key(&id)
                || node.is_focusable_by_default()
                || node.attributes().contains_key("tabindex")
            {
                self.tab_dirty.insert(id);
            }
            stack.extend(node.children());
        }
    }

    /// The `tabindex` of `node_id`. A node not connected to the tree's document,
    /// a text node, a node that has an ancestor (or itself) that is a template or has
    /// `display: none`, `inert` or a hiding `hidden`, and a `disabled` HTML button
    /// or input have none. An HTML button or input, or a link, whose `tabindex` is missing or
    /// unparsable has 0.
    pub(in crate::focus) fn tab_index_of(
        tree: &VirtualTree,
        cache: &StyleCache,
        node_id: NodeId,
    ) -> Option<i32> {
        let node = tree.get(node_id)?;
        if !tree.is_connected(node_id) {
            return None;
        }
        let hidden = tree.ancestors(node_id).any(|id| {
            hidden_by_attribute(tree, cache, id) || tree.get(id).is_some_and(|ancestor| {
                ancestor.is_template()
                    || ancestor.attributes().contains_key("inert")
                    || matches!(
                        cache.computed_style(tree, id).get("display"),
                        Some(AttributeValue::String(value)) if value.eq_ignore_ascii_case("none")
                    )
            })
        });
        let by_default = node.is_focusable_by_default();
        if !node.is_element() || hidden || is_disabled_control(node) {
            return None;
        }
        tree.explicit_tab_index(node_id).or(by_default.then_some(0))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::*;
    use gpui::TestAppContext;
    use inca_node::{remove_style, set_style};
    use rstest::rstest;
    use std::collections::HashSet;

    /// A rooted tree with one `button`, disabled when `disabled`.
    fn rooted_button(disabled: bool) -> (VirtualTree, NodeId) {
        let (mut tree, _) = rooted_tree(0);
        let id = tree.create_node("button").unwrap();
        tree.append_child(ROOT, id).unwrap();
        if disabled {
            tree.set_attribute(id, "disabled", true).unwrap();
        }
        (tree, id)
    }

    #[test]
    fn a_button_under_a_template_has_no_tab_index() {
        let (mut tree, button) = rooted_button(false);
        let template = tree.create_node("template").unwrap();
        tree.append_child(ROOT, template).unwrap();
        tree.append_child(template, button).unwrap();

        assert_eq!(
            FocusRegistry::tab_index_of(&tree, &StyleCache::new(), button),
            None
        );
    }

    #[test]
    fn a_text_node_and_a_display_none_node_have_no_tab_index() {
        let (tree, id) = tree_with("#text", Some(AttributeValue::from("0")));
        assert_eq!(
            FocusRegistry::tab_index_of(&tree, &StyleCache::new(), id),
            None
        );
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        set_style(&mut tree, id, "display", "none").unwrap();
        assert_eq!(
            FocusRegistry::tab_index_of(&tree, &StyleCache::new(), id),
            None
        );
        set_style(&mut tree, id, "display", "flex").unwrap();
        assert_eq!(
            FocusRegistry::tab_index_of(&tree, &StyleCache::new(), id),
            Some(0)
        );
    }

    #[test]
    fn a_comment_with_a_tabindex_has_no_tab_index() {
        let (tree, id) = tree_with("#comment", Some(AttributeValue::from("0")));
        assert_eq!(
            FocusRegistry::tab_index_of(&tree, &StyleCache::new(), id),
            None
        );
    }

    #[test]
    fn a_display_none_in_capitals_leaves_no_tab_index() {
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        set_style(&mut tree, id, "display", "NONE").unwrap();
        assert_eq!(
            FocusRegistry::tab_index_of(&tree, &StyleCache::new(), id),
            None
        );
    }

    #[rstest]
    #[case::display("display")]
    #[case::hidden("hidden")]
    #[case::inert("inert")]
    fn a_node_under_display_none_hidden_or_inert_has_no_tab_index(#[case] hide: &str) {
        let mut mismatches = Vec::new();
        let (mut tree, child) = tree_with("div", Some(AttributeValue::from("0")));
        let parent = tree.create_node("div").unwrap();
        tree.append_child(parent, child).unwrap();
        for (target, name) in [(child, "self"), (parent, "ancestor")] {
            if hide == "display" {
                set_style(&mut tree, target, "display", "none").unwrap();
            } else {
                tree.set_attribute(target, hide, true).unwrap();
            }
            if FocusRegistry::tab_index_of(&tree, &StyleCache::new(), child).is_some() {
                mismatches.push(format!("{hide} on {name} keeps the tab index"));
            }
            if hide == "display" {
                remove_style(&mut tree, target, "display").unwrap();
            } else {
                tree.remove_attribute(target, hide).unwrap();
            }
            if FocusRegistry::tab_index_of(&tree, &StyleCache::new(), child) != Some(0) {
                mismatches.push(format!("removing {hide} on {name} loses the tab index"));
            }
        }
        assert!(mismatches.is_empty(), "{mismatches:#?}");
    }

    /// A `<style>` element under the root with a rule that hides `.h`.
    fn hide_class_h(tree: &mut VirtualTree) {
        let style = tree.create_node("style").unwrap();
        tree.append_child(ROOT, style).unwrap();
        let css = tree.create_node("#text").unwrap();
        tree.set_data(css, ".h { display: none }").unwrap();
        tree.append_child(style, css).unwrap();
    }

    #[test]
    fn a_node_under_a_rule_display_none_has_no_tab_index() {
        let (mut tree, _) = rooted_tree(0);
        hide_class_h(&mut tree);
        let parent = tree.create_node("div").unwrap();
        tree.append_child(ROOT, parent).unwrap();
        tree.set_attribute(parent, "class", "h").unwrap();
        let child = tree.create_node("div").unwrap();
        tree.append_child(parent, child).unwrap();
        tree.set_attribute(child, "tabindex", "0").unwrap();

        assert_eq!(
            FocusRegistry::tab_index_of(&tree, &StyleCache::new(), child),
            None
        );
    }

    #[test]
    fn a_node_with_a_rule_display_none_has_no_tab_index() {
        let (mut tree, _) = rooted_tree(0);
        hide_class_h(&mut tree);
        let node = tree.create_node("div").unwrap();
        tree.append_child(ROOT, node).unwrap();
        tree.set_attribute(node, "class", "h").unwrap();
        tree.set_attribute(node, "tabindex", "0").unwrap();

        assert_eq!(
            FocusRegistry::tab_index_of(&tree, &StyleCache::new(), node),
            None
        );
    }

    #[test]
    fn hidden_with_an_own_display_or_until_found_keeps_the_tab_index() {
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        tree.set_attribute(id, "hidden", "").unwrap();
        assert_eq!(
            FocusRegistry::tab_index_of(&tree, &StyleCache::new(), id),
            None
        );
        set_style(&mut tree, id, "display", "flex").unwrap();
        assert_eq!(
            FocusRegistry::tab_index_of(&tree, &StyleCache::new(), id),
            Some(0)
        );
        remove_style(&mut tree, id, "display").unwrap();
        tree.set_attribute(id, "hidden", "until-found").unwrap();
        assert_eq!(
            FocusRegistry::tab_index_of(&tree, &StyleCache::new(), id),
            Some(0)
        );
    }

    const SVG: &str = "http://www.w3.org/2000/svg";

    #[rstest]
    #[case::button("button")]
    #[case::input("input")]
    fn an_svg_control_has_no_default_tab_index(#[case] tag: &str) {
        let (mut tree, _) = rooted_tree(0);
        let id = tree.create_element_ns(Some(SVG), None, tag).unwrap();
        tree.append_child(ROOT, id).unwrap();
        assert_eq!(
            FocusRegistry::tab_index_of(&tree, &StyleCache::new(), id),
            None,
            "{tag}"
        );
    }

    #[test]
    fn an_svg_template_ancestor_leaves_the_tab_index_of_its_subtree() {
        let (mut tree, button) = rooted_button(false);
        let template = tree.create_element_ns(Some(SVG), None, "template").unwrap();
        tree.append_child(ROOT, template).unwrap();
        tree.append_child(template, button).unwrap();
        assert_eq!(
            FocusRegistry::tab_index_of(&tree, &StyleCache::new(), button),
            Some(0)
        );
    }

    #[gpui::test]
    fn mark_subtree_dirty_queues_nodes_that_can_hold_focus(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, focusable) = tree_with("div", Some(AttributeValue::from("0")));
        let parent = tree.create_node("div").unwrap();
        let plain = tree.create_node("div").unwrap();
        let button = tree.create_node("button").unwrap();
        let input = tree.create_node("input").unwrap();
        let held = tree.create_node("div").unwrap();
        for child in [focusable, plain, button, input, held] {
            tree.append_child(parent, child).unwrap();
        }
        let mut registry = FocusRegistry::default();
        cx.update(|_, cx| registry.get_or_create(held, cx));
        registry.mark_subtree_dirty(&tree, parent);
        assert_eq!(
            registry.tab_dirty,
            HashSet::from([focusable, button, input, held])
        );
    }

    #[rstest]
    #[case::absent(None, Some(0))]
    #[case::positive(Some(AttributeValue::from("3")), Some(3))]
    #[case::negative(Some(AttributeValue::from("-1")), Some(-1))]
    #[case::not_a_number(Some(AttributeValue::from("abc")), Some(0))]
    #[case::boolean(Some(AttributeValue::Bool(true)), Some(0))]
    fn a_button_has_tab_index_0_unless_told_otherwise(
        #[case] value: Option<AttributeValue>,
        #[case] want: Option<i32>,
    ) {
        let (tree, id) = tree_with("button", value.clone());
        assert_eq!(
            FocusRegistry::tab_index_of(&tree, &StyleCache::new(), id),
            want,
            "{value:?}"
        );
    }

    #[test]
    fn an_input_has_tab_index_0_without_a_tabindex() {
        let (tree, id) = tree_with("input", None);
        assert_eq!(
            FocusRegistry::tab_index_of(&tree, &StyleCache::new(), id),
            Some(0)
        );
    }

    #[rstest]
    #[case::button("button", None)]
    #[case::input("input", None)]
    #[case::div("div", Some(2))]
    fn a_disabled_attribute_drops_the_tab_index_of_a_control_only(
        #[case] tag: &str,
        #[case] want: Option<i32>,
    ) {
        let (mut tree, id) = tree_with(tag, Some(AttributeValue::from("2")));
        tree.set_attribute(id, "disabled", true).unwrap();
        assert_eq!(
            FocusRegistry::tab_index_of(&tree, &StyleCache::new(), id),
            want,
            "{tag}"
        );
        tree.remove_attribute(id, "disabled").unwrap();
        assert_eq!(
            FocusRegistry::tab_index_of(&tree, &StyleCache::new(), id),
            Some(2),
            "{tag}"
        );
    }

    #[gpui::test]
    fn focus_on_a_disabled_button_is_ignored(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, id) = rooted_button(true);
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        registry.request_focus(id, false);

        let transitions =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        assert!(transitions.is_empty());
        assert_eq!(
            cx.update(|window, cx| registry.focused_node(window, cx)),
            None
        );
    }

    #[gpui::test]
    fn disabling_a_focused_button_reports_its_blur(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, id) = rooted_button(false);
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &[id], id);

        tree.set_attribute(id, "disabled", true).unwrap();
        registry.mark_tab_dirty(id);
        let transitions =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].blurred, Some(id));
        assert_eq!(transitions[0].focused, None);
    }

    #[gpui::test]
    fn an_enabled_button_takes_focus_again(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, id) = rooted_button(false);
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &[id], id);
        tree.set_attribute(id, "disabled", true).unwrap();
        registry.mark_tab_dirty(id);
        cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        tree.remove_attribute(id, "disabled").unwrap();
        registry.mark_tab_dirty(id);
        registry.request_focus(id, false);
        let transitions =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].blurred, None);
        assert_eq!(transitions[0].focused, Some(id));
    }

    #[test]
    fn tab_index_of_reads_the_default_focusability_of_the_kind() {
        let (mut tree, _) = rooted_tree(0);
        let with_href = tree.create_node("a").unwrap();
        let without = tree.create_node("a").unwrap();
        tree.append_child(ROOT, with_href).unwrap();
        tree.append_child(ROOT, without).unwrap();
        tree.set_attribute(with_href, "href", "u").unwrap();
        assert_eq!(
            FocusRegistry::tab_index_of(&tree, &StyleCache::new(), with_href),
            Some(0)
        );
        assert_eq!(
            FocusRegistry::tab_index_of(&tree, &StyleCache::new(), without),
            None
        );
    }
}
