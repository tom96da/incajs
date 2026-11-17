// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Inserting a list of nodes and texts in one checked step.

use std::collections::HashSet;

use super::{NodeId, NodeKind, TEXT_TAG, TreeError, VirtualNode, VirtualTree};

/// One argument of an insertion: a node of the tree, or a string that becomes a new text node.
pub enum InsertArg {
    Node(NodeId),
    Text(String),
}

impl InsertArg {
    fn node(&self) -> Option<NodeId> {
        match self {
            InsertArg::Node(id) => Some(*id),
            InsertArg::Text(_) => None,
        }
    }
}

/// Where an insertion puts its nodes, relative to the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// After the last child of the target.
    Append,
    /// Before the first child of the target that the insertion does not move.
    Prepend,
    /// Before the target, after its first preceding sibling that is no argument.
    Before,
    /// After the target, before its first following sibling that is no argument.
    After,
    /// In the place of the target, which leaves its parent unless it is an argument.
    ReplaceWith,
    /// As the only children of the target.
    ReplaceChildren,
}

/// A checked insertion that [`VirtualTree::apply_insertion`] carries out.
pub struct Insertion {
    parent: NodeId,
    group: Vec<Planned>,
    texts: Vec<String>,
    /// The child of `parent` that the group goes before, or the end.
    reference: Option<NodeId>,
    /// The children of `parent` that leave the tree.
    removed: Vec<NodeId>,
}

/// A member of the group of an insertion.
enum Planned {
    Node(NodeId),
    /// The index into the texts of the insertion.
    Text(usize),
}

impl Planned {
    fn node(&self) -> Option<NodeId> {
        match self {
            Planned::Node(id) => Some(*id),
            Planned::Text(_) => None,
        }
    }
}

impl Insertion {
    /// The ids of the existing nodes that the insertion moves or removes.
    #[must_use]
    pub fn leaving(&self) -> Vec<NodeId> {
        let moved = self.group.iter().filter_map(Planned::node);
        moved.chain(self.removed.iter().copied()).collect()
    }
}

/// What an applied insertion made and removed.
#[derive(Debug)]
pub struct Inserted {
    /// The ids of the new text nodes, in argument order.
    pub texts: Vec<NodeId>,
    /// The ids of the nodes that left the tree. A node that moved is not listed.
    pub removed: Vec<NodeId>,
}

impl VirtualTree {
    /// Checks the insertion of `nodes` at `placement` of `target` and plans it. Nothing changes,
    /// and no id is taken.
    ///
    /// A string argument becomes a new text node. With one argument the node itself goes in, and a
    /// fragment gives its children. With none or several, the nodes go in as one group in argument
    /// order: a fragment gives its children, a node given twice takes the place of its last
    /// occurrence, and a node below an earlier argument leaves it.
    ///
    /// Returns `Ok(None)` for `Before`, `After` and `ReplaceWith` on a target without a parent.
    ///
    /// # Errors
    ///
    /// The first check that fails returns:
    ///
    /// - [`TreeError::NodeNotFound`] if `target` or a node argument names no node.
    /// - [`TreeError::IdsExhausted`] if fewer ids are left than there are strings.
    /// - [`TreeError::DocumentInserted`] if one of several arguments is a document, and, after the
    ///   checks below, if the one argument is a document.
    /// - [`TreeError::LeafParent`] if the parent is a text node or a comment.
    /// - [`TreeError::WouldCycle`] if the group holds the parent or one of its ancestors.
    /// - [`TreeError::NotAChild`] for `After` and `ReplaceWith` when the following sibling is a
    ///   node that the insertion moves out of the parent.
    /// - [`TreeError::DocumentChild`] if the parent is a document and the insertion would give it
    ///   text or a second element.
    pub fn plan_insertion(
        &self,
        target: NodeId,
        placement: Placement,
        nodes: &[InsertArg],
    ) -> Result<Option<Insertion>, TreeError> {
        let target_node = self
            .nodes
            .get(&target)
            .ok_or(TreeError::NodeNotFound(target))?;
        let parent_id = match placement {
            Placement::Append | Placement::Prepend | Placement::ReplaceChildren => target,
            _ => match target_node.parent {
                Some(parent) => parent,
                None => return Ok(None),
            },
        };
        let arguments: HashSet<NodeId> = nodes.iter().filter_map(InsertArg::node).collect();
        let unknown = nodes
            .iter()
            .filter_map(InsertArg::node)
            .find(|id| !self.nodes.contains_key(id));
        if let Some(id) = unknown {
            return Err(TreeError::NodeNotFound(id));
        }
        let texts: Vec<String> = nodes
            .iter()
            .filter_map(|arg| match arg {
                InsertArg::Text(text) => Some(text.clone()),
                InsertArg::Node(_) => None,
            })
            .collect();
        // `allocate` never hands out `u32::MAX`, so this is the number of ids left.
        let left = u32::MAX - self.next_id;
        if u32::try_from(texts.len()).is_ok_and(|count| count > left) {
            return Err(TreeError::IdsExhausted);
        }
        let group = self.plan_group(nodes)?;
        let moves = nodes.len() != 1;
        let in_group: HashSet<NodeId> = group.iter().filter_map(Planned::node).collect();
        let parent = self
            .nodes
            .get(&parent_id)
            .ok_or(TreeError::NodeNotFound(parent_id))?;
        if parent.is_character_data() {
            return Err(TreeError::LeafParent(parent_id));
        }
        self.check_no_cycle(nodes, parent_id, &in_group)?;
        let (reference, removed) = Self::reference_and_removed(
            parent.children(),
            target,
            placement,
            &arguments,
            &in_group,
            parent_id,
        )?;
        if let [InsertArg::Node(id)] = nodes
            && self.nodes.get(id).is_some_and(VirtualNode::is_document)
        {
            return Err(TreeError::DocumentInserted(*id));
        }
        if parent.is_document() {
            let kinds = group.iter().map(|planned| match planned {
                Planned::Node(id) => self.nodes.get(id).map(VirtualNode::kind),
                Planned::Text(_) => Some(NodeKind::Text),
            });
            let replaced = (placement == Placement::ReplaceWith).then_some(target);
            let stays = |id: NodeId| !(moves && in_group.contains(&id)) && Some(id) != replaced;
            self.check_document_child(parent, kinds, stays, target)?;
        }
        Ok(Some(Insertion {
            parent: parent_id,
            group,
            texts,
            reference,
            removed,
        }))
    }

    /// Refuses a group that holds the parent or one of its ancestors. A template continues at its
    /// content.
    fn check_no_cycle(
        &self,
        nodes: &[InsertArg],
        parent_id: NodeId,
        in_group: &HashSet<NodeId>,
    ) -> Result<(), TreeError> {
        if let [InsertArg::Node(id)] = nodes {
            return if self.is_self_or_ancestor(*id, parent_id) {
                Err(TreeError::WouldCycle(*id))
            } else {
                Ok(())
            };
        }
        let mut current = Some(parent_id);
        while let Some(id) = current {
            if in_group.contains(&id) {
                return Err(TreeError::WouldCycle(id));
            }
            current = self
                .nodes
                .get(&id)
                .and_then(|node| node.parent.or(node.owner));
        }
        Ok(())
    }

    /// The group of an insertion: the nodes that go in, in order. A document among several
    /// arguments is refused here, and a document as the one argument at the end of the checks.
    fn plan_group(&self, nodes: &[InsertArg]) -> Result<Vec<Planned>, TreeError> {
        if let [arg] = nodes {
            return Ok(match arg {
                InsertArg::Node(id) => self
                    .nodes_to_move(*id)
                    .into_iter()
                    .map(Planned::Node)
                    .collect(),
                InsertArg::Text(_) => vec![Planned::Text(0)],
            });
        }
        let mut group = Vec::new();
        let mut members = HashSet::new();
        let mut texts = 0;
        for arg in nodes {
            let InsertArg::Node(id) = arg else {
                group.push(Planned::Text(texts));
                texts += 1;
                continue;
            };
            let node = self.nodes.get(id).ok_or(TreeError::NodeNotFound(*id))?;
            if node.is_document() {
                return Err(TreeError::DocumentInserted(*id));
            }
            if node.kind() == NodeKind::Fragment {
                for &child in node.children() {
                    if members.insert(child) {
                        group.push(Planned::Node(child));
                    }
                }
            } else {
                if !members.insert(*id) {
                    group.retain(|planned| !matches!(planned, Planned::Node(held) if held == id));
                }
                group.push(Planned::Node(*id));
            }
        }
        Ok(group)
    }

    /// The child of the parent that the group goes before, and the children that leave. `kids` are
    /// the children of the parent.
    fn reference_and_removed(
        kids: &[NodeId],
        target: NodeId,
        placement: Placement,
        arguments: &HashSet<NodeId>,
        in_group: &HashSet<NodeId>,
        parent: NodeId,
    ) -> Result<(Option<NodeId>, Vec<NodeId>), TreeError> {
        let free = |id: &&NodeId| !in_group.contains(*id);
        let at = kids.iter().position(|&kid| kid == target).unwrap_or(0);
        let after = |at: usize| -> Result<Option<NodeId>, TreeError> {
            let Some(next) = kids[at + 1..].iter().find(|id| !arguments.contains(*id)) else {
                return Ok(None);
            };
            if in_group.contains(next) {
                return Err(TreeError::NotAChild {
                    parent,
                    child: *next,
                });
            }
            Ok(Some(*next))
        };
        Ok(match placement {
            Placement::Append => (None, Vec::new()),
            Placement::Prepend => (kids.iter().find(free).copied(), Vec::new()),
            Placement::Before => {
                let start = kids[..at]
                    .iter()
                    .rposition(|id| !arguments.contains(id))
                    .map_or(0, |previous| previous + 1);
                (kids[start..].iter().find(free).copied(), Vec::new())
            }
            Placement::After => (after(at)?, Vec::new()),
            Placement::ReplaceWith if in_group.contains(&target) => (after(at)?, Vec::new()),
            Placement::ReplaceWith => (kids[at + 1..].iter().find(free).copied(), vec![target]),
            Placement::ReplaceChildren => (None, kids.iter().filter(free).copied().collect()),
        })
    }

    /// Carries out a planned insertion: creates the text nodes, removes the nodes that leave and
    /// puts the group in its place together. The plan has to come from the current state of the
    /// tree.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::IdsExhausted`] if the tree has fewer ids left than the plan needs. A
    /// plan made by [`plan_insertion`](Self::plan_insertion) from the current state never fails.
    pub fn apply_insertion(&mut self, insertion: Insertion) -> Result<Inserted, TreeError> {
        let Insertion {
            parent,
            group,
            texts,
            reference,
            removed,
        } = insertion;
        let mut text_ids = Vec::with_capacity(texts.len());
        for text in texts {
            let id = self.create_node(TEXT_TAG)?;
            self.set_attribute(id, "value", text)?;
            text_ids.push(id);
        }
        self.detach_all(&removed);
        for &id in &removed {
            self.orphan_if_free(id);
        }
        let ids: Vec<NodeId> = group
            .iter()
            .map(|planned| match planned {
                Planned::Node(id) => *id,
                Planned::Text(index) => text_ids[*index],
            })
            .collect();
        let mut by_parent: Vec<(NodeId, Vec<NodeId>)> = Vec::new();
        for &id in &ids {
            let Some(old) = self.nodes.get(&id).and_then(|node| node.parent) else {
                continue;
            };
            match by_parent.iter_mut().find(|(known, _)| *known == old) {
                Some((_, siblings)) => siblings.push(id),
                None => by_parent.push((old, vec![id])),
            }
        }
        for (_, siblings) in &by_parent {
            self.detach_all(siblings);
        }
        self.place(parent, ids, reference);
        Ok(Inserted {
            texts: text_ids,
            removed,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::collections::HashMap;

    use rstest::rstest;

    use super::*;
    use crate::tests::{Fixture, children, not_a_child};
    use crate::tree::{COMMENT_TAG, DOCUMENT_TAG, FRAGMENT_TAG};

    /// `p` holds `a, b, c, d` and `x` holds `y`, both under the root. The other nodes have no
    /// parent, and `doc` holds `r`. `id` finds a node by its name.
    struct Scene {
        tree: VirtualTree,
        ids: HashMap<&'static str, NodeId>,
    }

    impl Scene {
        fn id(&self, name: &str) -> NodeId {
            self.ids[name]
        }

        fn names(&self, name: &str) -> Vec<String> {
            names(&self.tree, self.id(name))
        }

        fn run(
            &mut self,
            target: &str,
            placement: Placement,
            nodes: &[InsertArg],
        ) -> Result<Option<Inserted>, TreeError> {
            run(&mut self.tree, self.ids[target], placement, nodes)
        }
    }

    fn named(tree: &mut VirtualTree, tag: &str, name: &str) -> NodeId {
        let id = tree.create_node(tag).unwrap();
        tree.set_attribute(id, "id", name).unwrap();
        id
    }

    fn scene() -> Scene {
        let mut f = Fixture::new();
        let mut ids = HashMap::new();
        for (name, parent) in [("p", f.root), ("x", f.root)] {
            ids.insert(name, f.add(parent, "div", name));
        }
        for name in ["a", "b", "c", "d"] {
            ids.insert(name, f.add(ids["p"], "span", name));
        }
        ids.insert("y", f.add(ids["x"], "span", "y"));
        let mut tree = f.tree;
        for (name, tag) in [
            ("e", "div"),
            ("txt", TEXT_TAG),
            ("k", COMMENT_TAG),
            ("doc", DOCUMENT_TAG),
            ("r", "div"),
        ] {
            ids.insert(name, named(&mut tree, tag, name));
        }
        tree.set_attribute(ids["txt"], "value", "t").unwrap();
        tree.append_child(ids["doc"], ids["r"]).unwrap();
        Scene { tree, ids }
    }

    type FragmentCall = fn(NodeId, NodeId, NodeId, NodeId) -> (NodeId, Placement, Vec<InsertArg>);
    type Call = fn(&Scene) -> (NodeId, Placement, Vec<InsertArg>);
    type Checked = fn(&Scene) -> (NodeId, Placement, Vec<InsertArg>, Result<(), TreeError>);
    type Failing = fn(&Scene) -> (NodeId, Placement, Vec<InsertArg>, TreeError);

    fn arg(s: &Scene, name: &str) -> InsertArg {
        InsertArg::Node(s.id(name))
    }

    fn n(id: NodeId) -> InsertArg {
        InsertArg::Node(id)
    }

    fn t(text: &str) -> InsertArg {
        InsertArg::Text(text.to_string())
    }

    fn run(
        tree: &mut VirtualTree,
        target: NodeId,
        placement: Placement,
        nodes: &[InsertArg],
    ) -> Result<Option<Inserted>, TreeError> {
        match tree.plan_insertion(target, placement, nodes)? {
            Some(insertion) => tree.apply_insertion(insertion).map(Some),
            None => Ok(None),
        }
    }

    fn apply(s: &mut Scene, call: Call) -> Inserted {
        let (target, placement, nodes) = call(s);
        run(&mut s.tree, target, placement, &nodes)
            .unwrap()
            .unwrap()
    }

    fn label(tree: &VirtualTree, id: NodeId) -> String {
        let node = tree.get(id).unwrap();
        if node.kind() == NodeKind::Text {
            format!("#t:{}", node.attributes()["value"].text())
        } else {
            node.attributes()["id"].text()
        }
    }

    fn labels(tree: &VirtualTree, ids: &[NodeId]) -> Vec<String> {
        ids.iter().map(|&id| label(tree, id)).collect()
    }

    fn names(tree: &VirtualTree, id: NodeId) -> Vec<String> {
        labels(tree, &children(tree, id))
    }

    /// The next id and the parent and children of every node.
    type Shape = (NodeId, Vec<(NodeId, Option<NodeId>, Vec<NodeId>)>);

    fn shape(tree: &VirtualTree) -> Shape {
        let mut nodes: Vec<_> = tree
            .nodes
            .values()
            .map(|node| (node.id(), node.parent(), node.children().to_vec()))
            .collect();
        nodes.sort();
        (tree.next_id, nodes)
    }

    #[rstest]
    #[case::append(|s: &Scene| (s.id("p"), Placement::Append, vec![arg(s, "y"), t("s")]), &["a", "b", "c", "d", "y", "#t:s"])]
    #[case::prepend(|s: &Scene| (s.id("p"), Placement::Prepend, vec![arg(s, "a"), t("s")]), &["a", "#t:s", "b", "c", "d"])]
    fn the_group_goes_where_the_member_puts_it(#[case] call: Call, #[case] expected: &[&str]) {
        let mut s = scene();

        let inserted = apply(&mut s, call);

        assert_eq!(s.names("p"), expected);
        assert_eq!(labels(&s.tree, &inserted.texts), ["#t:s"]);
    }

    #[rstest]
    #[case::string(|s: &Scene| (s.id("b"), Placement::Before, vec![t("s")]), &["a", "#t:s", "b", "c", "d"])]
    #[case::target_then_string(|s: &Scene| (s.id("b"), Placement::Before, vec![arg(s, "b"), t("s")]), &["a", "b", "#t:s", "c", "d"])]
    #[case::string_then_target(|s: &Scene| (s.id("b"), Placement::Before, vec![t("s"), arg(s, "b")]), &["a", "#t:s", "b", "c", "d"])]
    #[case::first_child_as_the_only_argument(|s: &Scene| (s.id("a"), Placement::Before, vec![arg(s, "a")]), &["a", "b", "c", "d"])]
    fn before_places_the_group_after_the_viable_previous_sibling(
        #[case] call: Call,
        #[case] expected: &[&str],
    ) {
        let mut s = scene();

        apply(&mut s, call);

        assert_eq!(s.names("p"), expected);
    }

    #[rstest]
    #[case::string(|s: &Scene| (s.id("b"), Placement::After, vec![t("s")]), &["a", "b", "#t:s", "c", "d"])]
    #[case::next_sibling_then_string(|s: &Scene| (s.id("b"), Placement::After, vec![arg(s, "c"), t("s")]), &["a", "b", "c", "#t:s", "d"])]
    #[case::string_then_target(|s: &Scene| (s.id("b"), Placement::After, vec![t("s"), arg(s, "b")]), &["a", "#t:s", "b", "c", "d"])]
    #[case::last_child_as_the_only_argument(|s: &Scene| (s.id("d"), Placement::After, vec![arg(s, "d")]), &["a", "b", "c", "d"])]
    fn after_places_the_group_before_the_viable_next_sibling(
        #[case] call: Call,
        #[case] expected: &[&str],
    ) {
        let mut s = scene();

        apply(&mut s, call);

        assert_eq!(s.names("p"), expected);
    }

    #[rstest]
    #[case::replace_with(|s: &Scene| (s.id("b"), Placement::ReplaceWith, vec![arg(s, "c"), t("s")]), &["a", "c", "#t:s", "d"], &["b"])]
    #[case::replace_with_the_target_among_the_arguments(|s: &Scene| (s.id("b"), Placement::ReplaceWith, vec![t("s"), arg(s, "b")]), &["a", "#t:s", "b", "c", "d"], &[])]
    #[case::replace_with_no_argument(|s: &Scene| (s.id("b"), Placement::ReplaceWith, vec![]), &["a", "c", "d"], &["b"])]
    #[case::replace_children_with_no_argument(|s: &Scene| (s.id("p"), Placement::ReplaceChildren, vec![]), &[], &["a", "b", "c", "d"])]
    #[case::replace_children_with_a_child(|s: &Scene| (s.id("p"), Placement::ReplaceChildren, vec![arg(s, "b"), t("s")]), &["b", "#t:s"], &["a", "c", "d"])]
    fn a_replacing_call_lists_the_nodes_that_the_group_does_not_keep(
        #[case] call: Call,
        #[case] expected: &[&str],
        #[case] removed: &[&str],
    ) {
        let mut s = scene();

        let inserted = apply(&mut s, call);

        assert_eq!(s.names("p"), expected);
        assert_eq!(labels(&s.tree, &inserted.removed), removed);
    }

    #[rstest]
    #[case::append(Placement::Append)]
    #[case::prepend(Placement::Prepend)]
    fn inserting_no_argument_changes_nothing(#[case] placement: Placement) {
        let mut s = scene();
        let before = (shape(&s.tree), s.tree.generation);

        let inserted = s.run("p", placement, &[]).unwrap().unwrap();

        assert_eq!((shape(&s.tree), s.tree.generation), before);
        assert!(inserted.texts.is_empty() && inserted.removed.is_empty());
    }

    #[rstest]
    #[case::before(Placement::Before)]
    #[case::after(Placement::After)]
    #[case::replace_with(Placement::ReplaceWith)]
    fn a_target_without_a_parent_gives_no_plan(#[case] placement: Placement) {
        let s = scene();

        let plan = s
            .tree
            .plan_insertion(s.id("e"), placement, &[t("s"), arg(&s, "a"), n(999)])
            .unwrap();

        assert!(plan.is_none());
    }

    #[test]
    fn a_fragment_argument_is_emptied_into_the_group() {
        let mut s = scene();
        let fragment = s.tree.create_node(FRAGMENT_TAG).unwrap();
        for name in ["k1", "k2"] {
            let kid = named(&mut s.tree, "span", name);
            s.tree.append_child(fragment, kid).unwrap();
        }

        s.run("p", Placement::Append, &[arg(&s, "x"), n(fragment)])
            .unwrap();

        assert_eq!(s.names("p"), ["a", "b", "c", "d", "x", "k1", "k2"]);
        assert_eq!(children(&s.tree, fragment), []);
        assert_eq!(s.tree.get(fragment).unwrap().parent(), None);
    }

    #[rstest]
    #[case::repeated_argument(|s: &Scene| vec![arg(s, "a"), t("s"), arg(s, "b"), arg(s, "a")], &["c", "d", "#t:s", "b", "a"], &["y"])]
    #[case::argument_below_an_earlier_one(|s: &Scene| vec![arg(s, "x"), arg(s, "y")], &["a", "b", "c", "d", "x", "y"], &[])]
    fn append_orders_the_group_by_the_last_occurrence_of_each_node(
        #[case] args: fn(&Scene) -> Vec<InsertArg>,
        #[case] expected: &[&str],
        #[case] in_x: &[&str],
    ) {
        let mut s = scene();
        let nodes = args(&s);

        s.run("p", Placement::Append, &nodes).unwrap();

        assert_eq!(s.names("p"), expected);
        assert_eq!(s.names("x"), in_x);
    }

    #[rstest]
    #[case::unknown_target(|_: &Scene| (999, Placement::Append, vec![], TreeError::NodeNotFound(999)))]
    #[case::unknown_argument(|s: &Scene| (s.id("p"), Placement::Append, vec![n(999)], TreeError::NodeNotFound(999)))]
    #[case::document_after_a_node_of_another_parent(|s: &Scene| (s.id("x"), Placement::Append, vec![arg(s, "a"), arg(s, "doc")], TreeError::DocumentInserted(s.id("doc"))))]
    #[case::document_into_a_parentless_element(|s: &Scene| (s.id("e"), Placement::Append, vec![arg(s, "doc")], TreeError::DocumentInserted(s.id("doc"))))]
    #[case::text_parent(|s: &Scene| (s.id("txt"), Placement::Append, vec![arg(s, "a")], TreeError::LeafParent(s.id("txt"))))]
    #[case::comment_parent(|s: &Scene| (s.id("k"), Placement::Append, vec![arg(s, "a")], TreeError::LeafParent(s.id("k"))))]
    #[case::ancestor_after_a_node_of_another_parent(|s: &Scene| (s.id("a"), Placement::Append, vec![arg(s, "x"), arg(s, "p")], TreeError::WouldCycle(s.id("p"))))]
    #[case::ancestor_of_the_parent_beside_a_target(|s: &Scene| (s.id("a"), Placement::Before, vec![arg(s, "x"), arg(s, "p")], TreeError::WouldCycle(s.id("p"))))]
    #[case::the_parent_as_the_one_argument(|s: &Scene| (s.id("p"), Placement::Append, vec![arg(s, "p")], TreeError::WouldCycle(s.id("p"))))]
    #[case::string_into_a_document(|s: &Scene| (s.id("doc"), Placement::Append, vec![t("s")], TreeError::DocumentChild(s.id("doc"))))]
    #[case::text_node_into_a_document(|s: &Scene| (s.id("doc"), Placement::Append, vec![arg(s, "txt")], TreeError::DocumentChild(s.id("doc"))))]
    #[case::two_elements_into_a_document(|s: &Scene| (s.id("doc"), Placement::Append, vec![arg(s, "e"), arg(s, "x")], TreeError::DocumentChild(s.id("doc"))))]
    #[case::element_into_a_document_with_one(|s: &Scene| (s.id("doc"), Placement::Append, vec![arg(s, "e")], TreeError::DocumentChild(s.id("doc"))))]
    fn a_failed_check_returns_its_error(#[case] call: Failing) {
        let mut s = scene();
        let (target, placement, nodes, expected) = call(&s);
        let before = shape(&s.tree);

        let outcome = run(&mut s.tree, target, placement, &nodes);

        assert_eq!(outcome.unwrap_err(), expected);
        assert_eq!(shape(&s.tree), before);
    }

    #[rstest]
    #[case::none_left(u32::MAX, 1)]
    #[case::one_left_for_two(u32::MAX - 1, 2)]
    fn a_call_that_needs_more_ids_than_are_left_fails(#[case] next: u32, #[case] strings: usize) {
        let mut s = scene();
        s.tree.set_next_id(next);
        let nodes: Vec<InsertArg> = (0..strings).map(|_| t("s")).collect();
        let before = shape(&s.tree);

        let outcome = s.run("p", Placement::Append, &nodes);

        assert_eq!(outcome.unwrap_err(), TreeError::IdsExhausted);
        assert_eq!(shape(&s.tree), before);
    }

    #[rstest]
    #[case::one_string(u32::MAX - 1, 1)]
    #[case::two_strings(u32::MAX - 2, 2)]
    fn a_call_whose_strings_take_the_last_free_ids_succeeds(
        #[case] next: u32,
        #[case] strings: usize,
    ) {
        let mut s = scene();
        s.tree.set_next_id(next);
        let nodes: Vec<InsertArg> = (0..strings).map(|_| t("s")).collect();

        let inserted = s.run("p", Placement::Append, &nodes).unwrap().unwrap();

        assert_eq!(inserted.texts.len(), strings);
    }

    #[rstest]
    #[case::element_and_comment(|s: &Scene| (s.id("doc"), Placement::Append, vec![arg(s, "r"), arg(s, "k")], Ok(())))]
    #[case::element_alone(|s: &Scene| (s.id("doc"), Placement::Append, vec![arg(s, "r")], Err(TreeError::DocumentChild(s.id("doc")))))]
    #[case::replace_with_a_new_element(|s: &Scene| (s.id("r"), Placement::ReplaceWith, vec![arg(s, "e")], Ok(())))]
    #[case::replace_with_itself(|s: &Scene| (s.id("r"), Placement::ReplaceWith, vec![arg(s, "r")], Ok(())))]
    #[case::replace_children_with_a_new_element(|s: &Scene| (s.id("doc"), Placement::ReplaceChildren, vec![arg(s, "e")], Err(TreeError::DocumentChild(s.id("doc")))))]
    #[case::replace_children_with_a_new_element_and_a_comment(|s: &Scene| (s.id("doc"), Placement::ReplaceChildren, vec![arg(s, "e"), arg(s, "k")], Err(TreeError::DocumentChild(s.id("doc")))))]
    #[case::replace_children_with_the_element(|s: &Scene| (s.id("doc"), Placement::ReplaceChildren, vec![arg(s, "r")], Err(TreeError::DocumentChild(s.id("doc")))))]
    #[case::replace_children_with_the_element_and_a_comment(|s: &Scene| (s.id("doc"), Placement::ReplaceChildren, vec![arg(s, "r"), arg(s, "k")], Ok(())))]
    fn a_document_keeps_to_one_element_that_the_call_leaves_in_place(#[case] call: Checked) {
        let mut s = scene();
        let (target, placement, nodes, expected) = call(&s);

        let outcome = run(&mut s.tree, target, placement, &nodes).map(|_| ());

        assert_eq!(outcome, expected);
    }

    /// A fragment `f` holding `c1, c2` and a new element `u`.
    fn fragment_family() -> (VirtualTree, [NodeId; 4]) {
        let mut tree = VirtualTree::new();
        let fragment = tree.create_node(FRAGMENT_TAG).unwrap();
        let [c1, c2] = ["c1", "c2"].map(|name| {
            let kid = named(&mut tree, "span", name);
            tree.append_child(fragment, kid).unwrap();
            kid
        });
        let u = named(&mut tree, "span", "u");
        (tree, [fragment, c1, c2, u])
    }

    #[rstest]
    #[case::append(|f: NodeId, _: NodeId, _: NodeId, u: NodeId| (f, Placement::Append, vec![n(u), n(f)]))]
    #[case::before_the_last_child(|f: NodeId, _: NodeId, c2: NodeId, u: NodeId| (c2, Placement::Before, vec![n(u), n(f)]))]
    #[case::before_the_first_child(|f: NodeId, c1: NodeId, _: NodeId, u: NodeId| (c1, Placement::Before, vec![n(u), n(f)]))]
    #[case::after_the_last_child(|f: NodeId, _: NodeId, c2: NodeId, u: NodeId| (c2, Placement::After, vec![n(u), n(f)]))]
    fn a_fragment_parent_among_the_arguments_gives_the_new_nodes_first(#[case] call: FragmentCall) {
        let (mut tree, [f, c1, c2, u]) = fragment_family();
        let (target, placement, nodes) = call(f, c1, c2, u);

        run(&mut tree, target, placement, &nodes).unwrap();

        assert_eq!(names(&tree, f), ["u", "c1", "c2"]);
    }

    #[test]
    fn after_refuses_a_next_sibling_that_the_fragment_argument_moves() {
        let (mut tree, [f, c1, c2, u]) = fragment_family();
        let before = shape(&tree);

        let outcome = run(&mut tree, c1, Placement::After, &[n(u), n(f)]);

        assert_eq!(outcome.unwrap_err(), not_a_child(f, c2));
        assert_eq!(shape(&tree), before);
    }

    #[rstest]
    #[case::one_argument(|_: NodeId, template: NodeId| vec![n(template)])]
    #[case::two_arguments(|other: NodeId, template: NodeId| vec![n(other), n(template)])]
    fn an_argument_that_owns_the_content_holding_the_parent_is_refused(
        #[case] args: fn(NodeId, NodeId) -> Vec<InsertArg>,
    ) {
        let mut tree = VirtualTree::new();
        let template = named(&mut tree, "template", "t");
        let inside = named(&mut tree, "span", "i");
        let loose = named(&mut tree, "span", "l");
        let content = tree.template_content(template).unwrap();
        tree.append_child(content, inside).unwrap();
        let nodes = args(loose, template);

        let outcome = run(&mut tree, inside, Placement::Append, &nodes);

        assert_eq!(outcome.unwrap_err(), TreeError::WouldCycle(template));
    }

    #[rstest]
    #[case::replace_children(|s: &Scene| (s.id("p"), Placement::ReplaceChildren, vec![t("s")]))]
    #[case::replace_with(|s: &Scene| (s.id("b"), Placement::ReplaceWith, vec![t("s")]))]
    fn a_sweep_after_the_call_frees_exactly_what_left(#[case] call: Call) {
        let mut s = scene();

        let inserted = apply(&mut s, call);
        let freed = s.tree.sweep();

        assert!(inserted.removed.iter().all(|id| freed.contains(id)));
        assert!(inserted.texts.iter().all(|&id| s.tree.get(id).is_some()));
    }

    #[test]
    fn replace_with_a_child_of_the_target_frees_only_the_target() {
        let mut s = scene();
        let (b, y) = (s.id("b"), s.id("y"));
        s.tree.append_child(b, y).unwrap();

        s.run("b", Placement::ReplaceWith, &[n(y)]).unwrap();

        assert_eq!(s.names("p"), ["a", "y", "c", "d"]);
        assert!(s.names("b").is_empty());
        let freed = s.tree.sweep();
        assert!(freed.contains(&b) && !freed.contains(&y));
    }

    #[test]
    fn replace_children_with_a_node_inside_a_child_empties_the_child() {
        let mut s = scene();
        let (c, y) = (s.id("c"), s.id("y"));
        s.tree.append_child(c, y).unwrap();

        let nodes = [n(y), arg(&s, "a")];
        s.run("p", Placement::ReplaceChildren, &nodes).unwrap();

        assert_eq!(s.names("p"), ["y", "a"]);
        assert!(s.names("c").is_empty());
    }
}
