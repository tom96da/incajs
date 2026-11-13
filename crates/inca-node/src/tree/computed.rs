// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The computed style of a node: the rules of the `<style>` elements merged with the node's own
//! style, the order of shorthand and longhand keys, the cache and the warnings.

use std::borrow::Cow;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::rc::Rc;

use super::{AttributeValue, NodeId, StyleMap, TEXT_TAG, VirtualNode, VirtualTree};
use crate::selector::StateFlags;
use crate::stylesheet::{Rule, StyleSheet};

/// The longhands that a shorthand key overrides when it is applied after them.
const SHORTHANDS: &[(&str, &[&str])] = &[
    (
        "padding",
        &[
            "padding_x",
            "padding_y",
            "padding_top",
            "padding_right",
            "padding_bottom",
            "padding_left",
        ],
    ),
    ("padding_x", &["padding_left", "padding_right"]),
    ("padding_y", &["padding_top", "padding_bottom"]),
    (
        "margin",
        &[
            "margin_x",
            "margin_y",
            "margin_top",
            "margin_right",
            "margin_bottom",
            "margin_left",
        ],
    ),
    ("margin_x", &["margin_left", "margin_right"]),
    ("margin_y", &["margin_top", "margin_bottom"]),
    ("overflow", &["overflow_x", "overflow_y"]),
];

/// How many longhands `key` overrides: 0 for a key that is no shorthand.
fn covers(key: &str) -> usize {
    SHORTHANDS
        .iter()
        .find(|(shorthand, _)| *shorthand == key)
        .map_or(0, |(_, longhands)| longhands.len())
}

/// Sets `key`, after removing the keys it overrides.
fn apply(map: &mut StyleMap, key: &str, value: &AttributeValue) {
    if let Some((_, longhands)) = SHORTHANDS.iter().find(|(shorthand, _)| *shorthand == key) {
        for longhand in *longhands {
            map.shift_remove(*longhand);
        }
    }
    map.insert(key.to_owned(), value.clone());
}

/// The entries of `props`, shorthands before the longhands they override.
fn shorthands_first<'a>(
    props: impl IntoIterator<Item = (&'a String, &'a AttributeValue)>,
) -> Vec<(&'a String, &'a AttributeValue)> {
    let mut entries: Vec<_> = props.into_iter().collect();
    entries.sort_by_key(|(key, _)| (Reverse(covers(key)), key.as_str()));
    entries
}

/// The sheets of the connected `<style>` elements, kept until the tree changes.
#[derive(Debug, Default)]
pub(super) struct SheetCache {
    /// The generation of the tree the cache was built for.
    generation: Option<u64>,
    /// The sheets in tree order.
    sheets: Vec<Rc<StyleSheet>>,
    /// The state flags of the rules of every sheet.
    flags: StateFlags,
    /// The text each `<style>` element had when its sheet was parsed.
    parsed: HashMap<NodeId, (String, Rc<StyleSheet>)>,
    /// The warning lines of the parses nobody has taken yet.
    warnings: Vec<String>,
}

impl VirtualTree {
    /// The connected `<style>` elements in tree order. None while the tree has no document or root.
    fn style_elements(&self) -> Vec<NodeId> {
        let mut found = Vec::new();
        let mut pending: Vec<NodeId> = self.document.or(self.root).into_iter().collect();
        while let Some(id) = pending.pop() {
            let Some(node) = self.get(id) else { continue };
            if node.is_style() {
                found.push(id);
                // The children of a `<style>` element are its text.
                continue;
            }
            pending.extend(node.children().iter().rev());
        }
        found
    }

    /// The data of the text children of `id`, in order.
    fn child_text(&self, id: NodeId) -> String {
        self.get(id)
            .into_iter()
            .flat_map(VirtualNode::children)
            .filter_map(|&child| self.get(child))
            .filter(|child| child.tag_name() == TEXT_TAG)
            .map(VirtualNode::data)
            .collect()
    }

    /// Collects the sheets again when the tree changed. A sheet whose text is unchanged keeps its
    /// parse.
    fn refresh_sheets(&self) {
        let mut cache = self.style_cache.borrow_mut();
        if cache.generation == Some(self.generation) {
            return;
        }
        cache.generation = Some(self.generation);
        let mut sheets = Vec::new();
        let mut parsed = HashMap::new();
        for id in self.style_elements() {
            let text = self.child_text(id);
            let entry = match cache.parsed.remove(&id) {
                Some((old, sheet)) if old == text => (old, sheet),
                _ => {
                    let (sheet, warnings) = StyleSheet::parse(&text);
                    cache.warnings.extend(warnings);
                    (text, Rc::new(sheet))
                }
            };
            sheets.push(Rc::clone(&entry.1));
            parsed.insert(id, entry);
        }
        cache.flags = sheets
            .iter()
            .flat_map(|sheet| &sheet.rules)
            .fold(StateFlags::default(), |all, rule| {
                all.merge(rule.selectors.flags())
            });
        cache.sheets = sheets;
        cache.parsed = parsed;
    }

    /// The state flags of the connected sheets.
    pub(super) fn sheet_flags(&self) -> StateFlags {
        self.refresh_sheets();
        self.style_cache.borrow().flags
    }

    /// The warning lines of the sheet parses since the last call. A sheet with unchanged text
    /// adds none.
    pub fn take_style_warnings(&self) -> Vec<String> {
        self.refresh_sheets();
        std::mem::take(&mut self.style_cache.borrow_mut().warnings)
    }

    /// The style of `id` after the rules of the connected `<style>` elements apply: the matching
    /// declarations by specificity, then sheet order, then rule order, and the node's own style
    /// last. An empty own value is ignored. A declaration of a shorthand replaces the longhands set
    /// before it. A node that no rule styles, an unknown id and a tree without a `<style>` element
    /// give the node's own map.
    #[must_use]
    pub fn computed_style(&self, id: NodeId) -> Cow<'_, StyleMap> {
        let Some(node) = self.get(id) else {
            return Cow::Owned(StyleMap::new());
        };
        self.refresh_sheets();
        let sheets = self.style_cache.borrow().sheets.clone();
        if sheets.is_empty() {
            return Cow::Borrowed(node.style_props());
        }
        let mut matched: Vec<(u32, usize, usize, &Rule)> = Vec::new();
        for (sheet_index, sheet) in sheets.iter().enumerate() {
            for (rule_index, rule) in sheet.rules.iter().enumerate() {
                if let Some(specificity) = rule.selectors.specificity_of_match(self, id) {
                    matched.push((specificity, sheet_index, rule_index, rule));
                }
            }
        }
        matched.sort_by_key(|&(specificity, sheet_index, rule_index, _)| {
            (specificity, sheet_index, rule_index)
        });
        if matched.is_empty() {
            return Cow::Borrowed(node.style_props());
        }
        let mut computed = StyleMap::new();
        for (_, _, _, rule) in matched {
            for (key, value) in &rule.declarations {
                apply(&mut computed, key, value);
            }
        }
        for (key, value) in shorthands_first(node.style_props()) {
            // An empty inline value removes the inline declaration.
            if value.as_str() == Some("") {
                continue;
            }
            apply(&mut computed, key, value);
        }
        Cow::Owned(computed)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::build_spec;
    use crate::tests::{Fixture, SVG, num, text};

    fn padding_top(f: &Fixture, node: NodeId) -> Option<f64> {
        build_spec(&f.tree, node).unwrap().style.padding.top
    }

    #[test]
    fn a_rule_applies_its_declarations_to_a_matching_node() {
        let mut f = Fixture::new();
        f.sheet(".a { width: 40px }");
        let a = f.boxed("a");
        let b = f.boxed("b");

        assert_eq!(f.computed(a, "width"), Some(text("40px")));
        assert_eq!(f.computed(b, "width"), None);
    }

    #[test]
    fn a_higher_specificity_beats_a_later_rule() {
        let mut f = Fixture::new();
        f.sheet("#a { width: 1px } .a { width: 2px }");
        let a = f.boxed("a");

        assert_eq!(f.computed(a, "width"), Some(text("1px")));
    }

    #[test]
    fn the_later_sheet_wins_at_equal_specificity() {
        let mut f = Fixture::new();
        f.sheet(".a { width: 1px }");
        f.sheet(".a { width: 2px }");
        let a = f.boxed("a");

        assert_eq!(f.computed(a, "width"), Some(text("2px")));
    }

    #[test]
    fn the_inline_style_beats_every_rule() {
        let mut f = Fixture::new();
        f.sheet("#a.a { width: 1px }");
        let a = f.boxed("a");
        f.tree.set_style(a, "width", 50.0).unwrap();

        assert_eq!(f.computed(a, "width"), Some(num(50.0)));
    }

    #[test]
    fn an_empty_inline_value_keeps_the_rule_value() {
        let mut f = Fixture::new();
        f.sheet(".a { display: flex }");
        let a = f.boxed("a");
        f.tree.set_style(a, "display", "").unwrap();

        assert_eq!(f.computed(a, "display"), Some(text("flex")));
    }

    #[test]
    fn an_empty_inline_shorthand_keeps_the_rule_longhands() {
        let mut f = Fixture::new();
        f.sheet(".a { padding-top: 8px }");
        let a = f.boxed("a");
        f.tree.set_style(a, "padding", "").unwrap();

        assert_eq!(padding_top(&f, a), Some(8.0));
    }

    #[test]
    fn an_empty_inline_shorthand_leaves_the_rule_sides_beside_an_inline_side() {
        let mut f = Fixture::new();
        f.sheet(".a { padding: 4px }");
        let a = f.boxed("a");
        f.tree.set_style(a, "padding", "").unwrap();
        f.tree.set_style(a, "padding_top", 2.0).unwrap();
        let spec = build_spec(&f.tree, a).unwrap();

        assert_eq!(spec.style.padding.top, Some(2.0));
        assert_eq!(spec.style.padding.left, Some(4.0));
    }

    #[test]
    fn a_rule_shorthand_replaces_an_earlier_rule_longhand() {
        let mut f = Fixture::new();
        f.sheet(".a { padding-top: 8px } .a { padding: 4px }");
        let a = f.boxed("a");

        assert_eq!(padding_top(&f, a), Some(4.0));
    }

    #[test]
    fn an_inline_shorthand_replaces_a_rule_longhand() {
        let mut f = Fixture::new();
        f.sheet(".a { padding-top: 8px }");
        let a = f.boxed("a");
        f.tree.set_style(a, "padding", 4.0).unwrap();

        assert_eq!(padding_top(&f, a), Some(4.0));
    }

    #[test]
    fn an_inline_axis_replaces_a_rule_side() {
        let mut f = Fixture::new();
        f.sheet(".a { padding-left: 8px }");
        let a = f.boxed("a");
        f.tree.set_style(a, "padding_x", 4.0).unwrap();

        assert_eq!(
            build_spec(&f.tree, a).unwrap().style.padding.left,
            Some(4.0)
        );
    }

    #[rstest]
    #[case::margin("margin-top: 1px; margin: 2px", "margin_top")]
    #[case::margin_x("margin-left: 1px; margin-x: 2px", "margin_left")]
    #[case::margin_y("margin-bottom: 1px; margin-y: 2px", "margin_bottom")]
    #[case::padding_x("padding-left: 1px; padding-x: 2px", "padding_left")]
    #[case::padding_y("padding-top: 1px; padding-y: 2px", "padding_top")]
    #[case::overflow("overflow-x: hidden; overflow: scroll", "overflow_x")]
    fn a_later_shorthand_removes_its_longhand(#[case] declarations: &str, #[case] longhand: &str) {
        let mut f = Fixture::new();
        f.sheet(&format!(".a {{ {declarations} }}"));
        let a = f.boxed("a");

        assert_eq!(f.computed(a, longhand), None);
    }

    #[test]
    fn a_longhand_after_its_shorthand_wins_on_its_side() {
        let mut f = Fixture::new();
        f.sheet(".a { padding: 4px; padding-top: 8px }");
        let a = f.boxed("a");
        let spec = build_spec(&f.tree, a).unwrap();

        assert_eq!(spec.style.padding.top, Some(8.0));
        assert_eq!(spec.style.padding.left, Some(4.0));
    }

    #[test]
    fn shorthands_come_before_the_longhands_they_override() {
        // The reverse of the expected order.
        let props: Vec<(String, AttributeValue)> = [
            "width",
            "padding_top",
            "padding_x",
            "overflow",
            "padding",
            "margin",
        ]
        .map(|key| (key.to_owned(), num(1.0)))
        .into();

        let keys: Vec<&str> = shorthands_first(props.iter().map(|(key, value)| (key, value)))
            .into_iter()
            .map(|(key, _)| key.as_str())
            .collect();

        assert_eq!(
            keys,
            [
                "margin",
                "padding",
                "overflow",
                "padding_x",
                "padding_top",
                "width"
            ]
        );
    }

    #[test]
    fn an_svg_style_element_applies_its_rules() {
        let mut f = Fixture::new();
        let style = f.tree.create_element_ns(Some(SVG), None, "style").unwrap();
        let css = f.tree.create_node("#text").unwrap();
        f.attr(css, "value", ".a { width: 40px }");
        f.tree.append_child(style, css).unwrap();
        f.tree.append_child(f.root, style).unwrap();
        let a = f.boxed("a");

        assert_eq!(f.computed(a, "width"), Some(text("40px")));
    }

    #[test]
    fn a_style_element_applies_once_it_is_connected() {
        let mut f = Fixture::new();
        let (style, _) = f.sheet(".a { width: 1px }");
        let a = f.boxed("a");
        f.tree.remove_child(f.root, style).unwrap();
        let detached = f.computed(a, "width");
        f.tree.append_child(f.root, style).unwrap();

        assert_eq!(detached, None);
        assert_eq!(f.computed(a, "width"), Some(text("1px")));
    }

    #[test]
    fn editing_the_sheet_text_changes_the_next_read() {
        let mut f = Fixture::new();
        let (_, text_node) = f.sheet(".a { width: 1px }");
        let a = f.boxed("a");
        let before = f.computed(a, "width");
        f.attr(text_node, "value", ".a { width: 2px }");

        assert_eq!(before, Some(text("1px")));
        assert_eq!(f.computed(a, "width"), Some(text("2px")));
    }

    #[test]
    fn a_sheet_warns_once_while_its_text_stays() {
        let mut f = Fixture::new();
        f.sheet(".a { bogus: 1 }");
        let a = f.boxed("a");
        let first = f.tree.take_style_warnings();
        f.tree.set_style(a, "width", 1.0).unwrap();

        assert_eq!(first, ["ignoring unknown style key `bogus`"]);
        assert!(f.tree.take_style_warnings().is_empty());
    }

    #[test]
    fn a_node_without_a_sheet_in_the_tree_borrows_its_inline_map() {
        let mut f = Fixture::new();
        let a = f.boxed("a");
        f.tree.set_style(a, "width", 1.0).unwrap();

        assert!(matches!(f.tree.computed_style(a), Cow::Borrowed(map) if map.len() == 1));
    }

    #[test]
    fn a_node_no_rule_styles_borrows_its_inline_map() {
        let mut f = Fixture::new();
        f.sheet(".a { width: 1px }");
        let b = f.boxed("b");

        assert!(matches!(f.tree.computed_style(b), Cow::Borrowed(_)));
    }

    #[test]
    fn a_style_tag_in_capitals_holds_no_rules() {
        let mut f = Fixture::new();
        let style = f.add(f.root, "STYLE", "sheet");
        let text_node = f.tree.create_node(TEXT_TAG).unwrap();
        f.attr(text_node, "value", ".a { width: 1px }");
        f.tree.append_child(style, text_node).unwrap();
        let a = f.boxed("a");

        assert_eq!(f.computed(a, "width"), None);
    }

    #[test]
    fn a_comment_child_of_a_style_element_is_not_sheet_text() {
        let mut f = Fixture::new();
        let (style, _) = f.sheet(".a { height: 2px }");
        let comment = f.tree.create_node("#comment").unwrap();
        f.attr(comment, "value", ".a { width: 9px }");
        f.tree.append_child(style, comment).unwrap();
        let a = f.boxed("a");

        assert_eq!(f.computed(a, "width"), None);
        assert_eq!(f.computed(a, "height"), Some(text("2px")));
    }

    #[test]
    fn a_style_element_inside_a_style_element_holds_no_rules() {
        let mut f = Fixture::new();
        let (outer, _) = f.sheet("");
        let inner = f.tree.create_node("style").unwrap();
        let text_node = f.tree.create_node(TEXT_TAG).unwrap();
        f.attr(text_node, "value", ".a { width: 1px }");
        f.tree.append_child(inner, text_node).unwrap();
        f.tree.append_child(outer, inner).unwrap();
        let a = f.boxed("a");

        assert_eq!(f.computed(a, "width"), None);
    }

    #[test]
    fn sheets_apply_in_tree_order_not_creation_order() {
        let mut f = Fixture::new();
        let (first, _) = f.sheet(".a { width: 1px }");
        let (second, _) = f.sheet(".a { width: 2px }");
        f.tree.insert_before(f.root, second, Some(first)).unwrap();
        let a = f.boxed("a");

        assert_eq!(f.computed(a, "width"), Some(text("1px")));
    }

    #[test]
    fn a_style_applies_while_the_root_hangs_under_the_document() {
        let (mut f, document) = Fixture::with_document();
        f.sheet(".a { width: 1px }");
        let a = f.boxed("a");
        let hung = f.computed(a, "width");
        f.tree.remove_child(document, f.root).unwrap();

        assert_eq!(hung, Some(text("1px")));
        assert_eq!(f.computed(a, "width"), None);
    }
}
