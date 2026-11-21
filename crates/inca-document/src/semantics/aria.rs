// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The ARIA table and the element references an element holds.

use std::collections::{HashMap, HashSet};

use inca_node::tree::{NodeId, TreeError, VirtualTree};

/// How a member of the ARIA table reads its attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AriaKind {
    /// The attribute value as text.
    Text,
    /// The element whose ID is the attribute value.
    Element,
    /// The elements whose IDs are the whitespace separated tokens of the attribute value.
    Elements,
}

/// A property of an element and the attribute it reflects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AriaReflection {
    /// The name of the property.
    pub idl: &'static str,
    /// The name of the attribute.
    pub attribute: &'static str,
    /// How the attribute is read.
    pub kind: AriaKind,
}

const fn text(idl: &'static str, attribute: &'static str) -> AriaReflection {
    AriaReflection {
        idl,
        attribute,
        kind: AriaKind::Text,
    }
}

const fn element(idl: &'static str, attribute: &'static str) -> AriaReflection {
    AriaReflection {
        idl,
        attribute,
        kind: AriaKind::Element,
    }
}

const fn elements(idl: &'static str, attribute: &'static str) -> AriaReflection {
    AriaReflection {
        idl,
        attribute,
        kind: AriaKind::Elements,
    }
}

/// The 52 members of `ARIAMixin` in the order of the standard.
pub const ARIA_REFLECTIONS: [AriaReflection; 52] = [
    text("role", "role"),
    element("ariaActiveDescendantElement", "aria-activedescendant"),
    text("ariaAtomic", "aria-atomic"),
    text("ariaAutoComplete", "aria-autocomplete"),
    text("ariaBrailleLabel", "aria-braillelabel"),
    text("ariaBrailleRoleDescription", "aria-brailleroledescription"),
    text("ariaBusy", "aria-busy"),
    text("ariaChecked", "aria-checked"),
    text("ariaColCount", "aria-colcount"),
    text("ariaColIndex", "aria-colindex"),
    text("ariaColIndexText", "aria-colindextext"),
    text("ariaColSpan", "aria-colspan"),
    elements("ariaControlsElements", "aria-controls"),
    text("ariaCurrent", "aria-current"),
    elements("ariaDescribedByElements", "aria-describedby"),
    text("ariaDescription", "aria-description"),
    elements("ariaDetailsElements", "aria-details"),
    text("ariaDisabled", "aria-disabled"),
    elements("ariaErrorMessageElements", "aria-errormessage"),
    text("ariaExpanded", "aria-expanded"),
    elements("ariaFlowToElements", "aria-flowto"),
    text("ariaHasPopup", "aria-haspopup"),
    text("ariaHidden", "aria-hidden"),
    text("ariaInvalid", "aria-invalid"),
    text("ariaKeyShortcuts", "aria-keyshortcuts"),
    text("ariaLabel", "aria-label"),
    elements("ariaLabelledByElements", "aria-labelledby"),
    text("ariaLevel", "aria-level"),
    text("ariaLive", "aria-live"),
    text("ariaModal", "aria-modal"),
    text("ariaMultiLine", "aria-multiline"),
    text("ariaMultiSelectable", "aria-multiselectable"),
    text("ariaOrientation", "aria-orientation"),
    elements("ariaOwnsElements", "aria-owns"),
    text("ariaPlaceholder", "aria-placeholder"),
    text("ariaPosInSet", "aria-posinset"),
    text("ariaPressed", "aria-pressed"),
    text("ariaReadOnly", "aria-readonly"),
    text("ariaRelevant", "aria-relevant"),
    text("ariaRequired", "aria-required"),
    text("ariaRoleDescription", "aria-roledescription"),
    text("ariaRowCount", "aria-rowcount"),
    text("ariaRowIndex", "aria-rowindex"),
    text("ariaRowIndexText", "aria-rowindextext"),
    text("ariaRowSpan", "aria-rowspan"),
    text("ariaSelected", "aria-selected"),
    text("ariaSetSize", "aria-setsize"),
    text("ariaSort", "aria-sort"),
    text("ariaValueMax", "aria-valuemax"),
    text("ariaValueMin", "aria-valuemin"),
    text("ariaValueNow", "aria-valuenow"),
    text("ariaValueText", "aria-valuetext"),
];

/// The row of the table for the attribute `attribute`.
#[must_use]
pub fn aria_reflection(attribute: &str) -> Option<&'static AriaReflection> {
    ARIA_REFLECTIONS
        .iter()
        .find(|row| row.attribute == attribute)
}

const ASCII_WHITESPACE: [char; 5] = [' ', '\t', '\n', '\x0c', '\r'];

/// The element references of the `aria` attributes of a tree.
pub trait AriaReferences {
    /// The elements the attribute `attribute` of `node` refers to, in order. A reference set with
    /// [`set_aria_elements`](Self::set_aria_elements) resolves to the nodes that are descendants
    /// of the root of `node`, and to nothing while `node` has no parent. Without one, the
    /// attribute value names the elements by ID inside the root of `node`. `None` for an unknown
    /// node, an attribute of kind [`AriaKind::Text`] or outside the table, and an absent
    /// attribute.
    fn aria_elements(&self, node: NodeId, attribute: &str) -> Option<Vec<NodeId>>;

    /// Sets the attribute `attribute` of `node` to the empty text and keeps `ids` as the elements
    /// it refers to, or removes the attribute and the reference for `None`. A write to an
    /// attribute outside the table or of kind [`AriaKind::Text`] changes nothing.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `node` names no node.
    fn set_aria_elements(
        &mut self,
        node: NodeId,
        attribute: &str,
        ids: Option<Vec<NodeId>>,
    ) -> Result<(), TreeError>;
}

impl AriaReferences for VirtualTree {
    fn aria_elements(&self, node: NodeId, attribute: &str) -> Option<Vec<NodeId>> {
        let row = aria_reflection(attribute)?;
        let element = self.get(node)?;
        let root = self.ancestors(node).last()?;
        if let Some(ids) = element.aria_refs(row.attribute) {
            if element.parent().is_none() {
                return Some(Vec::new());
            }
            return Some(
                ids.iter()
                    .copied()
                    .filter(|&id| id != root && self.ancestors(id).last() == Some(root))
                    .collect(),
            );
        }
        let value = element.attributes().get(row.attribute)?.text();
        let tokens: Vec<&str> = match row.kind {
            AriaKind::Text => return None,
            AriaKind::Element => vec![value.as_str()],
            AriaKind::Elements => value.split(ASCII_WHITESPACE).collect(),
        };
        Some(elements_with_ids(self, root, &tokens))
    }

    fn set_aria_elements(
        &mut self,
        node: NodeId,
        attribute: &str,
        ids: Option<Vec<NodeId>>,
    ) -> Result<(), TreeError> {
        let Some(row) = aria_reflection(attribute).filter(|row| row.kind != AriaKind::Text) else {
            return Ok(());
        };
        match ids {
            Some(ids) => {
                self.set_attribute(node, row.attribute, "")?;
                self.node_mut(node)?.set_aria_refs(row.attribute, ids);
            }
            None => self.remove_attribute(node, row.attribute)?,
        }
        Ok(())
    }
}

/// For each of `ids` that names an element of the tree under `root`, `root` included, the first
/// such element in tree order, in the order of `ids`. The empty text matches nothing. The
/// content of a template is its own tree. One walk serves all of `ids`.
fn elements_with_ids(tree: &VirtualTree, root: NodeId, ids: &[&str]) -> Vec<NodeId> {
    let mut found: HashMap<&str, NodeId> = HashMap::new();
    let wanted: HashSet<&str> = ids.iter().copied().filter(|id| !id.is_empty()).collect();
    let mut pending = vec![root];
    while let Some(current) = pending.pop() {
        if found.len() == wanted.len() {
            break;
        }
        let Some(node) = tree.get(current) else {
            break;
        };
        if node.is_element()
            && let Some(value) = node.attributes().get("id")
            && let Some(&id) = wanted.get(value.text().as_str())
        {
            found.entry(id).or_insert(current);
        }
        pending.extend(node.children().iter().rev());
    }
    ids.iter().filter_map(|id| found.get(id).copied()).collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use inca_node::tree::AttributeValue;

    /// A `div` root under a document with the children `names`, each with an `id` of its name.
    fn page(names: &[&str]) -> (VirtualTree, NodeId, Vec<NodeId>) {
        let mut tree = VirtualTree::new();
        let document = tree.create_node("#document").unwrap();
        tree.set_document(document);
        let root = tree.create_node("div").unwrap();
        tree.append_child(document, root).unwrap();
        let children = names
            .iter()
            .map(|name| {
                let child = tree.create_node("span").unwrap();
                tree.set_attribute(child, "id", *name).unwrap();
                tree.append_child(root, child).unwrap();
                child
            })
            .collect();
        (tree, root, children)
    }

    #[test]
    fn the_table_holds_the_52_members_of_the_draft_with_role_first() {
        let got: Vec<_> = ARIA_REFLECTIONS
            .iter()
            .map(|row| (row.idl, row.attribute, row.kind))
            .collect();

        assert_eq!(
            got,
            [
                ("role", "role", AriaKind::Text),
                (
                    "ariaActiveDescendantElement",
                    "aria-activedescendant",
                    AriaKind::Element
                ),
                ("ariaAtomic", "aria-atomic", AriaKind::Text),
                ("ariaAutoComplete", "aria-autocomplete", AriaKind::Text),
                ("ariaBrailleLabel", "aria-braillelabel", AriaKind::Text),
                (
                    "ariaBrailleRoleDescription",
                    "aria-brailleroledescription",
                    AriaKind::Text
                ),
                ("ariaBusy", "aria-busy", AriaKind::Text),
                ("ariaChecked", "aria-checked", AriaKind::Text),
                ("ariaColCount", "aria-colcount", AriaKind::Text),
                ("ariaColIndex", "aria-colindex", AriaKind::Text),
                ("ariaColIndexText", "aria-colindextext", AriaKind::Text),
                ("ariaColSpan", "aria-colspan", AriaKind::Text),
                ("ariaControlsElements", "aria-controls", AriaKind::Elements),
                ("ariaCurrent", "aria-current", AriaKind::Text),
                (
                    "ariaDescribedByElements",
                    "aria-describedby",
                    AriaKind::Elements
                ),
                ("ariaDescription", "aria-description", AriaKind::Text),
                ("ariaDetailsElements", "aria-details", AriaKind::Elements),
                ("ariaDisabled", "aria-disabled", AriaKind::Text),
                (
                    "ariaErrorMessageElements",
                    "aria-errormessage",
                    AriaKind::Elements
                ),
                ("ariaExpanded", "aria-expanded", AriaKind::Text),
                ("ariaFlowToElements", "aria-flowto", AriaKind::Elements),
                ("ariaHasPopup", "aria-haspopup", AriaKind::Text),
                ("ariaHidden", "aria-hidden", AriaKind::Text),
                ("ariaInvalid", "aria-invalid", AriaKind::Text),
                ("ariaKeyShortcuts", "aria-keyshortcuts", AriaKind::Text),
                ("ariaLabel", "aria-label", AriaKind::Text),
                (
                    "ariaLabelledByElements",
                    "aria-labelledby",
                    AriaKind::Elements
                ),
                ("ariaLevel", "aria-level", AriaKind::Text),
                ("ariaLive", "aria-live", AriaKind::Text),
                ("ariaModal", "aria-modal", AriaKind::Text),
                ("ariaMultiLine", "aria-multiline", AriaKind::Text),
                (
                    "ariaMultiSelectable",
                    "aria-multiselectable",
                    AriaKind::Text
                ),
                ("ariaOrientation", "aria-orientation", AriaKind::Text),
                ("ariaOwnsElements", "aria-owns", AriaKind::Elements),
                ("ariaPlaceholder", "aria-placeholder", AriaKind::Text),
                ("ariaPosInSet", "aria-posinset", AriaKind::Text),
                ("ariaPressed", "aria-pressed", AriaKind::Text),
                ("ariaReadOnly", "aria-readonly", AriaKind::Text),
                ("ariaRelevant", "aria-relevant", AriaKind::Text),
                ("ariaRequired", "aria-required", AriaKind::Text),
                (
                    "ariaRoleDescription",
                    "aria-roledescription",
                    AriaKind::Text
                ),
                ("ariaRowCount", "aria-rowcount", AriaKind::Text),
                ("ariaRowIndex", "aria-rowindex", AriaKind::Text),
                ("ariaRowIndexText", "aria-rowindextext", AriaKind::Text),
                ("ariaRowSpan", "aria-rowspan", AriaKind::Text),
                ("ariaSelected", "aria-selected", AriaKind::Text),
                ("ariaSetSize", "aria-setsize", AriaKind::Text),
                ("ariaSort", "aria-sort", AriaKind::Text),
                ("ariaValueMax", "aria-valuemax", AriaKind::Text),
                ("ariaValueMin", "aria-valuemin", AriaKind::Text),
                ("ariaValueNow", "aria-valuenow", AriaKind::Text),
                ("ariaValueText", "aria-valuetext", AriaKind::Text),
            ]
        );
    }

    #[test]
    fn the_names_of_the_table_are_unique() {
        let idl: HashSet<_> = ARIA_REFLECTIONS.iter().map(|row| row.idl).collect();
        let attributes: HashSet<_> = ARIA_REFLECTIONS.iter().map(|row| row.attribute).collect();

        assert_eq!((idl.len(), attributes.len()), (52, 52));
    }

    #[test]
    fn an_attribute_outside_the_table_has_no_row() {
        assert!(aria_reflection("aria-label").is_some());
        assert!(aria_reflection("ariaLabel").is_none());
    }

    #[test]
    fn an_id_with_two_matches_resolves_to_the_first_in_tree_order() {
        let (mut tree, root, kids) = page(&["a", "b"]);
        let nested = tree.create_node("i").unwrap();
        tree.set_attribute(nested, "id", "b").unwrap();
        tree.append_child(kids[0], nested).unwrap();
        tree.set_attribute(root, "aria-labelledby", "b").unwrap();

        assert_eq!(
            tree.aria_elements(root, "aria-labelledby"),
            Some(vec![nested])
        );
    }

    #[test]
    fn a_token_with_no_match_is_left_out_of_a_list() {
        let (mut tree, root, kids) = page(&["a", "b"]);
        tree.set_attribute(root, "aria-labelledby", " a  zz\tb ")
            .unwrap();

        assert_eq!(
            tree.aria_elements(root, "aria-labelledby"),
            Some(vec![kids[0], kids[1]])
        );
    }

    #[test]
    fn an_element_in_another_tree_is_not_found() {
        let (mut tree, root, _) = page(&[]);
        let other = tree.create_node("div").unwrap();
        tree.set_attribute(other, "id", "x").unwrap();
        tree.set_attribute(root, "aria-labelledby", "x").unwrap();

        assert_eq!(tree.aria_elements(root, "aria-labelledby"), Some(vec![]));
    }

    #[test]
    fn a_detached_element_finds_the_id_of_its_own_child() {
        let mut tree = VirtualTree::new();
        let host = tree.create_node("div").unwrap();
        let child = tree.create_node("span").unwrap();
        tree.set_attribute(child, "id", "y").unwrap();
        tree.append_child(host, child).unwrap();
        tree.set_attribute(host, "aria-activedescendant", "y")
            .unwrap();

        assert_eq!(
            tree.aria_elements(host, "aria-activedescendant"),
            Some(vec![child])
        );
    }

    #[test]
    fn the_single_kind_takes_the_whole_value_as_one_id() {
        let (mut tree, root, kids) = page(&["a", "a b"]);
        tree.set_attribute(root, "aria-activedescendant", "a b")
            .unwrap();

        assert_eq!(
            tree.aria_elements(root, "aria-activedescendant"),
            Some(vec![kids[1]])
        );
    }

    #[test]
    fn an_empty_value_matches_no_element_even_with_an_empty_id() {
        let (mut tree, root, kids) = page(&["x"]);
        tree.set_attribute(kids[0], "id", "").unwrap();
        tree.set_attribute(root, "aria-activedescendant", "")
            .unwrap();

        assert_eq!(
            tree.aria_elements(root, "aria-activedescendant"),
            Some(vec![])
        );
    }

    #[test]
    fn an_absent_attribute_reads_none() {
        let (tree, root, _) = page(&[]);

        assert_eq!(tree.aria_elements(root, "aria-labelledby"), None);
    }

    #[test]
    fn a_text_kind_attribute_reads_none() {
        let (mut tree, root, _) = page(&[]);
        tree.set_attribute(root, "aria-label", "x").unwrap();

        assert_eq!(tree.aria_elements(root, "aria-label"), None);
    }

    #[test]
    fn an_unknown_node_reads_none() {
        let (tree, _, _) = page(&[]);

        assert_eq!(tree.aria_elements(9999, "aria-labelledby"), None);
    }

    #[test]
    fn a_stored_reference_keeps_its_order() {
        let (mut tree, root, kids) = page(&["a", "b"]);

        tree.set_aria_elements(root, "aria-owns", Some(vec![kids[1], kids[0]]))
            .unwrap();

        assert_eq!(
            tree.aria_elements(root, "aria-owns"),
            Some(vec![kids[1], kids[0]])
        );
    }

    #[test]
    fn a_stored_reference_leaves_out_a_node_under_another_root() {
        let (mut tree, root, kids) = page(&["a"]);
        let other = tree.create_node("div").unwrap();
        let inner = tree.create_node("i").unwrap();
        tree.append_child(other, inner).unwrap();

        tree.set_aria_elements(root, "aria-owns", Some(vec![inner, other, kids[0]]))
            .unwrap();

        assert_eq!(tree.aria_elements(root, "aria-owns"), Some(vec![kids[0]]));
    }

    #[test]
    fn a_stored_reference_leaves_out_a_freed_node() {
        let (mut tree, root, kids) = page(&["a", "b"]);
        tree.set_aria_elements(root, "aria-owns", Some(kids.clone()))
            .unwrap();

        tree.destroy_node(kids[0]);

        assert_eq!(tree.aria_elements(root, "aria-owns"), Some(vec![kids[1]]));
    }

    #[test]
    fn an_element_without_a_parent_resolves_a_stored_reference_to_nothing() {
        let mut tree = VirtualTree::new();
        let host = tree.create_node("div").unwrap();
        let child = tree.create_node("span").unwrap();
        tree.append_child(host, child).unwrap();

        tree.set_aria_elements(host, "aria-owns", Some(vec![child]))
            .unwrap();

        assert_eq!(tree.aria_elements(host, "aria-owns"), Some(vec![]));
    }

    #[test]
    fn set_attribute_clears_the_reference() {
        let (mut tree, root, kids) = page(&["a", "b"]);
        tree.set_aria_elements(root, "aria-owns", Some(vec![kids[1]]))
            .unwrap();

        tree.set_attribute(root, "aria-owns", "a").unwrap();

        assert_eq!(tree.aria_elements(root, "aria-owns"), Some(vec![kids[0]]));
    }

    #[test]
    fn remove_attribute_clears_the_reference() {
        let (mut tree, root, kids) = page(&["a", "b"]);
        tree.set_aria_elements(root, "aria-owns", Some(vec![kids[1]]))
            .unwrap();

        tree.remove_attribute(root, "aria-owns").unwrap();
        tree.node_mut(root)
            .unwrap()
            .attributes
            .insert("aria-owns".to_owned(), "a".into());

        assert_eq!(tree.aria_elements(root, "aria-owns"), Some(vec![kids[0]]));
    }

    #[test]
    fn a_write_of_another_attribute_keeps_the_reference() {
        let (mut tree, root, kids) = page(&["a", "b"]);
        tree.set_aria_elements(root, "aria-owns", Some(vec![kids[1]]))
            .unwrap();

        tree.set_attribute(root, "aria-controls", "a").unwrap();
        tree.remove_attribute(root, "aria-flowto").unwrap();

        assert_eq!(tree.aria_elements(root, "aria-owns"), Some(vec![kids[1]]));
    }

    #[test]
    fn setting_ids_sets_an_empty_attribute_and_none_removes_it() {
        let (mut tree, root, kids) = page(&["a"]);

        tree.set_aria_elements(root, "aria-owns", Some(kids))
            .unwrap();
        let set = tree
            .get(root)
            .unwrap()
            .attributes()
            .get("aria-owns")
            .cloned();
        tree.set_aria_elements(root, "aria-owns", None).unwrap();

        assert_eq!(set.as_ref().map(AttributeValue::text), Some(String::new()));
        assert!(
            !tree
                .get(root)
                .unwrap()
                .attributes()
                .contains_key("aria-owns")
        );
        assert_eq!(tree.aria_elements(root, "aria-owns"), None);
    }

    #[test]
    fn a_set_on_a_text_attribute_changes_nothing() {
        let (mut tree, root, kids) = page(&["a"]);

        tree.set_aria_elements(root, "aria-label", Some(kids))
            .unwrap();

        assert!(
            !tree
                .get(root)
                .unwrap()
                .attributes()
                .contains_key("aria-label")
        );
    }

    #[test]
    fn a_set_on_an_unknown_node_fails() {
        let mut tree = VirtualTree::new();

        assert_eq!(
            tree.set_aria_elements(9, "aria-owns", None),
            Err(TreeError::NodeNotFound(9))
        );
    }

    #[test]
    fn a_clone_copies_the_attribute_and_no_reference() {
        let (mut tree, root, kids) = page(&["a"]);
        tree.set_aria_elements(root, "aria-owns", Some(kids))
            .unwrap();

        let copy = tree.clone_subtree(root, false).unwrap()[0].1;

        assert_eq!(
            tree.get(copy)
                .unwrap()
                .attributes()
                .get("aria-owns")
                .map(AttributeValue::text),
            Some(String::new())
        );
        assert_eq!(tree.get(copy).unwrap().aria_refs("aria-owns"), None);
    }
}
