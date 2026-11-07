// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The nesting depth of a parsed fragment.
//!
//! The sink counts the open elements down to every element it receives. A new element or comment
//! whose parent is deeper than the cap lands beside the parent, so the chain of first children
//! stops at the cap. Nodes that the adoption agency moves take the depth of the place they land in.

use html5ever::{local_name, ns};

use super::sink::{Handle, HtmlSink, node_of};
use crate::tree::NodeId;

/// The number of open elements, the root included, that a parent can have before a new element or
/// comment lands beside the parent. An HTML `template` lands beside it one level earlier.
const MAX_STACK_DEPTH: u32 = 512;

impl HtmlSink<'_> {
    /// Ends the token the tree builder is processing.
    pub(super) fn end_token(&self) {
        drop(self.attached.take());
    }

    /// The deepest open table section or row that `table` holds, or `table`. A node foster parented
    /// before `table` takes the depth of this node.
    fn current_in(&self, table: Handle) -> Handle {
        let mut current = table;
        while let Some(&section) = self.last_section.borrow().get(&current) {
            if self.closed.borrow().contains(&section) {
                break;
            }
            current = section;
        }
        current
    }

    fn is_section(&self, handle: Handle) -> bool {
        self.names.borrow().get(&handle).is_some_and(|name| {
            name.ns == ns!(html)
                && matches!(
                    name.local,
                    local_name!("tbody")
                        | local_name!("thead")
                        | local_name!("tfoot")
                        | local_name!("tr")
                )
        })
    }

    fn is_html_template(&self, handle: Handle) -> bool {
        self.names
            .borrow()
            .get(&handle)
            .is_some_and(|name| name.ns == ns!(html) && name.local == local_name!("template"))
    }

    /// The number of open elements down to `handle`.
    fn depth_of(&self, handle: Handle) -> u32 {
        self.stack_depth.borrow().get(&handle).copied().unwrap_or(0)
    }

    /// Gives `node` the open-element depth `depth`, and the same to the chain of clones below it
    /// down to the first node that was counted before.
    fn recount(&self, node: Handle, depth: u32) {
        let (mut current, mut depth) = (node, depth);
        loop {
            let counted = self
                .stack_depth
                .borrow_mut()
                .insert(current, depth)
                .is_some();
            let only_child =
                node_of(current).and_then(|id| match self.tree.borrow().get(id)?.children() {
                    [only] => Some(Handle::from(*only)),
                    _ => None,
                });
            match only_child {
                Some(next) if !counted => (current, depth) = (next, depth + 1),
                _ => return,
            }
        }
    }

    /// The parent and the sibling to insert before for `child`. A child that is already counted
    /// or holds children is a node the adoption agency moves, and a child put before `before` is
    /// foster parented: both stay where they are. Any other child goes to the parent of `parent`
    /// and is appended there once `parent` is below the depth cap, unless `parent` was put into
    /// the tree while the current token is processed. A foster parented child takes the depth of
    /// the open node inside the table `before`.
    pub(super) fn landing(
        &self,
        parent_id: NodeId,
        before: Option<Handle>,
        child: Handle,
    ) -> (NodeId, Option<NodeId>) {
        let parent = Handle::from(parent_id);
        let before_id = before.and_then(node_of);
        let moved = self.stack_depth.borrow().contains_key(&child)
            || node_of(child).is_some_and(|id| self.tree.borrow().first_child(id).is_some());
        if moved {
            self.attached.borrow_mut().insert(child);
            let counted = self.stack_depth.borrow().get(&parent).copied();
            if let Some(depth) = counted {
                self.recount(child, depth + 1);
            }
            return (parent_id, before_id);
        }
        let depth = self.depth_of(before.map_or(parent, |table| self.current_in(table)));
        self.stack_depth.borrow_mut().insert(child, depth + 1);
        if self.is_section(child) {
            self.last_section.borrow_mut().insert(parent, child);
        }
        self.attached.borrow_mut().insert(child);
        let limit = if self.is_html_template(child) {
            MAX_STACK_DEPTH
        } else {
            MAX_STACK_DEPTH + 1
        };
        let placed = (parent_id, before_id);
        if before.is_some() || depth < limit {
            return placed;
        }
        let anchor = self
            .content_owner
            .borrow()
            .get(&parent)
            .copied()
            .unwrap_or(parent);
        if self.attached.borrow().contains(&anchor) {
            return placed;
        }
        let beside = node_of(anchor).and_then(|id| self.tree.borrow().get(id)?.parent());
        beside.map_or(placed, |id| (id, None))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::html::tests::parsed;
    use crate::html::{ParsedHtml, ParsedNode};
    use crate::tests::node;
    use crate::tree::{COMMENT_TAG, NodeId, TEXT_TAG, VirtualTree};

    /// The number of ancestors of `id` below `fragment`, counting `id` itself.
    fn level(tree: &VirtualTree, fragment: NodeId, id: NodeId) -> usize {
        let mut level = 0;
        let mut current = id;
        while current != fragment {
            level += 1;
            current = tree.get(current).unwrap().parent().unwrap();
        }
        level
    }

    /// The depth of the first-child chain of elements.
    fn chain_depth(tree: &VirtualTree, fragment: NodeId) -> usize {
        let mut depth = 0;
        let mut current = fragment;
        while let Some(child) = tree.first_child(current) {
            if matches!(tree.get(child).unwrap().tag_name(), TEXT_TAG | COMMENT_TAG) {
                break;
            }
            depth += 1;
            current = child;
        }
        depth
    }

    /// The element `steps` levels down the first-child chain.
    fn chain_node(tree: &VirtualTree, fragment: NodeId, steps: usize) -> NodeId {
        let mut current = fragment;
        for _ in 0..steps {
            current = tree.first_child(current).unwrap();
        }
        current
    }

    fn child_tags(tree: &VirtualTree, id: NodeId) -> Vec<String> {
        let children = tree.get(id).unwrap().children();
        children
            .iter()
            .map(|&child| tree.get(child).unwrap().tag_name().to_owned())
            .collect()
    }

    fn next_tag(tree: &VirtualTree, id: NodeId) -> Option<String> {
        let next = tree.next_sibling(id)?;
        Some(tree.get(next).unwrap().tag_name().to_owned())
    }

    #[rstest]
    #[case::below_the_cap(511, 511)]
    #[case::at_the_cap(512, 512)]
    #[case::above_the_cap(600, 512)]
    fn the_depth_of_a_chain_stops_at_the_cap(#[case] repeats: usize, #[case] expected: usize) {
        let (tree, html) = parsed(Some("div"), &"<div>".repeat(repeats));

        assert_eq!(chain_depth(&tree, html.fragment), expected);
    }

    #[test]
    fn elements_past_the_cap_stay_in_the_result() {
        let (_, html) = parsed(Some("div"), &"<div>".repeat(600));

        assert_eq!(html.nodes.len(), 600);
    }

    #[test]
    fn text_past_the_cap_stays_in_its_element() {
        let markup = format!("{}t", "<div>".repeat(600));
        let (tree, html) = parsed(Some("div"), &markup);

        let text = entry(&html, TEXT_TAG).id;
        assert_eq!(level(&tree, html.fragment, text), 513);
    }

    #[rstest]
    #[case::divs_and_spans("<div><span>")]
    #[case::tables("<table><tr><td>")]
    fn mixed_tags_stop_at_the_cap_too(#[case] unit: &str) {
        let (tree, html) = parsed(Some("div"), &unit.repeat(300));

        assert_eq!(chain_depth(&tree, html.fragment), 512);
    }

    #[test]
    fn a_row_of_an_implied_section_stays_in_it_past_the_cap() {
        let (tree, html) = parsed(Some("div"), &"<table><tr><td>".repeat(130));

        let last = chain_node(&tree, html.fragment, 511);
        assert_eq!(child_tags(&tree, last), ["td", "table", "tbody"]);
    }

    #[test]
    fn a_row_of_a_written_section_lands_beside_it_past_the_cap() {
        let (tree, html) = parsed(Some("div"), &"<table><tbody><tr><td>".repeat(130));

        let last = chain_node(&tree, html.fragment, 511);
        let section = tree.get(last).unwrap().children()[2];
        assert_eq!(tree.first_child(section), None);
    }

    fn template_markup(repeats: usize) -> String {
        format!("{}<template><p>x</p></template>", "<div>".repeat(repeats))
    }

    fn entry<'a>(html: &'a ParsedHtml, tag: &str) -> &'a ParsedNode {
        html.nodes.iter().find(|entry| entry.tag == tag).unwrap()
    }

    #[test]
    fn a_template_below_the_cap_stays_inside_its_parent() {
        let (tree, html) = parsed(Some("div"), &template_markup(510));

        let template = entry(&html, "template").id;
        assert_eq!(level(&tree, html.fragment, template), 511);
    }

    #[test]
    fn a_template_below_the_cap_keeps_its_content() {
        let (tree, html) = parsed(Some("div"), &template_markup(510));

        let template = entry(&html, "template").id;
        let content = tree.template_content(template).unwrap();
        assert_eq!(child_tags(&tree, content), ["p"]);
    }

    #[rstest]
    #[case::one_level_early(511)]
    #[case::at_the_cap(512)]
    fn a_template_near_the_cap_lands_beside_its_parent(#[case] repeats: usize) {
        let (tree, html) = parsed(Some("div"), &template_markup(repeats));

        let template = entry(&html, "template").id;
        assert_eq!(level(&tree, html.fragment, template), repeats);
    }

    #[test]
    fn a_template_beside_its_parent_has_an_empty_content() {
        let (tree, html) = parsed(Some("div"), &template_markup(512));

        let template = entry(&html, "template").id;
        let content = tree.template_content(template).unwrap();
        assert!(tree.get(content).unwrap().children().is_empty());
    }

    #[test]
    fn a_foreign_template_follows_the_cap_of_any_element() {
        let markup = format!("{}<svg><template>", "<div>".repeat(510));
        let (tree, html) = parsed(Some("div"), &markup);

        let template = entry(&html, "template").id;
        assert_eq!(level(&tree, html.fragment, template), 512);
    }

    #[rstest]
    #[case::one_level_early(511)]
    #[case::at_the_cap(512)]
    #[case::far_below_the_cap(600)]
    fn the_children_of_a_flattened_template_land_beside_it(#[case] repeats: usize) {
        let (tree, html) = parsed(Some("div"), &template_markup(repeats));

        let template = entry(&html, "template").id;
        let beside = tree.get(template).unwrap().parent();
        assert_eq!(entry(&html, "p").parent, beside);
    }

    #[test]
    fn a_comment_below_the_cap_stays_inside_its_parent() {
        let markup = format!("{}<!--c-->", "<div>".repeat(511));
        let (tree, html) = parsed(Some("div"), &markup);

        let comment = entry(&html, COMMENT_TAG).id;
        assert_eq!(level(&tree, html.fragment, comment), 512);
    }

    #[rstest]
    #[case::at_the_cap(512)]
    #[case::far_below_the_cap(600)]
    fn a_comment_past_the_cap_lands_beside_its_parent(#[case] repeats: usize) {
        let markup = format!("{}<!--c-->", "<div>".repeat(repeats));
        let (tree, html) = parsed(Some("div"), &markup);

        let comment = entry(&html, COMMENT_TAG).id;
        assert_eq!(level(&tree, html.fragment, comment), 512);
    }

    #[rstest]
    #[case::at_the_cap(512)]
    #[case::far_below_the_cap(598)]
    fn a_fostered_element_lands_directly_before_its_table(#[case] repeats: usize) {
        let markup = format!("{}<table><b>x</b><tr><td>y", "<div>".repeat(repeats));
        let (tree, html) = parsed(Some("div"), &markup);

        let fostered = entry(&html, "b").id;
        assert_eq!(next_tag(&tree, fostered), Some("table".to_owned()));
    }

    #[rstest]
    #[case::in_a_section(508, "<table><tbody>")]
    #[case::in_a_row(507, "<table><tr>")]
    fn a_fostered_element_counts_the_open_section_for_its_children(
        #[case] repeats: usize,
        #[case] opening: &str,
    ) {
        let markup = format!("{}{opening}<b><i><u>x", "<div>".repeat(repeats));
        let (tree, html) = parsed(Some("div"), &markup);

        let fostered = entry(&html, "b").id;
        assert_eq!(child_tags(&tree, fostered), ["i", "u"]);
    }

    #[test]
    fn a_fostered_element_counts_the_table_once_its_section_is_closed() {
        let markup = format!(
            "{}<table><tbody><tr></tr></tbody><b><i><u>x",
            "<div>".repeat(508)
        );
        let (tree, html) = parsed(Some("div"), &markup);

        let fostered = entry(&html, "b").id;
        assert_eq!(child_tags(&tree, fostered), ["i"]);
    }

    #[test]
    fn an_empty_furthest_block_stays_in_the_common_ancestor() {
        let markup = format!("{}<b><p></b>x", "<div>".repeat(512));
        let (tree, html) = parsed(Some("div"), &markup);

        let block = entry(&html, "p").id;
        assert_eq!(level(&tree, html.fragment, block), 513);
    }

    #[test]
    fn a_clone_that_holds_the_furthest_block_stays_in_the_common_ancestor() {
        let markup = format!("{}<b><i>1<p>2</b>3</i>4", "<div>".repeat(597));
        let (tree, html) = parsed(Some("div"), &markup);

        let block = entry(&html, "p").id;
        let before = tree.previous_sibling(block).unwrap();
        assert_eq!(tree.get(before).unwrap().tag_name(), "i");
    }

    /// The number of ancestors of the deepest node, the result fragment excluded.
    fn deepest_level(html: &ParsedHtml) -> usize {
        let mut levels = std::collections::HashMap::from([(html.fragment, 0)]);
        for entry in &html.nodes {
            levels.insert(entry.id, levels[&entry.parent.unwrap()] + 1);
        }
        levels.into_values().max().unwrap()
    }

    #[rstest]
    #[case::repeated_adoption("<b><p>x</b>", 515)]
    #[case::clones_of_nested_formatting("<b><i><u><p>x</b></p>", 519)]
    #[case::anchor_with_a_block("<a>1<div>2</a>", 516)]
    fn misnested_formatting_markup_nests_as_deep_as_its_open_elements(
        #[case] unit: &str,
        #[case] expected: usize,
    ) {
        let markup = format!("{unit}{}", "<div>".repeat(508)).repeat(4);
        let (_, html) = parsed(Some("div"), &markup);

        assert_eq!(deepest_level(&html), expected);
    }

    #[rstest]
    #[case::one_clone("<b><i><div>x</b>", 526)]
    #[case::two_clones("<b><i><u><div>x</b>", 532)]
    #[case::three_clones("<b><i><u><s><div>x</b>", 538)]
    fn clones_of_a_block_adoption_nest_as_deep_as_their_open_elements(
        #[case] unit: &str,
        #[case] expected: usize,
    ) {
        let markup = format!("{unit}{}", "<span>".repeat(500)).repeat(8);
        let (_, html) = parsed(Some("div"), &markup);

        assert_eq!(deepest_level(&html), expected);
    }

    #[test]
    fn misnested_formatting_markup_at_the_cap_stays_as_deep_as_its_open_elements() {
        let unit = format!("<b><p>x</b>{}", "<div>".repeat(512));
        let markup = format!("{}{}", "<div>".repeat(512), unit.repeat(4));
        let (_, html) = parsed(Some("div"), &markup);

        assert_eq!(deepest_level(&html), 515);
    }

    #[test]
    fn the_furthest_block_takes_the_depth_of_its_new_place() {
        let markup = format!("{}<a>1<div>2</a><div><div><i>3</i>", "<div>".repeat(509));
        let (_, html) = parsed(Some("div"), &markup);

        assert_eq!(deepest_level(&html), 513);
    }

    fn open_tag(name: &str) -> String {
        format!("<{name}>")
    }

    fn close_tag(name: &str) -> String {
        format!("</{name}>")
    }

    /// `count` start tags of `b`, each with a different `id`.
    fn distinct_bolds(count: usize) -> String {
        (0..count)
            .map(|id| format!("<b id={id}>"))
            .collect::<Vec<_>>()
            .concat()
    }

    fn cap() -> String {
        "<div>".repeat(600)
    }

    #[rstest]
    #[case::attributes_stop_the_copy_limit(
        format!("{}<p>{}</p>x", cap(), distinct_bolds(16)),
        528
    )]
    #[case::twelve_names_reopened_twice(
        format!(
            "{}{}",
            cap(),
            "<p><b><i><u><s><em><strong><small><big><tt><code><font><strike></p>x".repeat(2)
        ),
        535
    )]
    #[case::reopened_in_a_table_cell(
        format!("{}<table><tbody><tr><td><b id=1><i id=2><u id=3></td></tr></table>x", cap()),
        513
    )]
    #[case::anchor_start_repairs(format!("{}{}", cap(), "<a>1<div>2".repeat(8)), 521)]
    #[case::nobr_start_repairs(format!("{}{}", cap(), "<nobr>1<div>2<nobr>".repeat(8)), 522)]
    #[case::eight_repairs_for_one_end_tag(
        format!("{}<b>{}x</b>{}", cap(), "<i><u><s><div>".repeat(8), "<span>".repeat(100)),
        531
    )]
    #[case::select_between_repairs(
        format!("{}{}", cap(), "<select><b><i>x</b><option>y".repeat(8)),
        517
    )]
    fn reopened_and_repaired_formatting_nests_as_deep_as_its_open_elements(
        #[case] markup: String,
        #[case] expected: usize,
    ) {
        let (_, html) = parsed(Some("div"), &markup);

        assert_eq!(deepest_level(&html), expected);
    }

    /// Markup from a fixed seed, with the counts that bound its depth.
    struct Generated {
        markup: String,
        /// The formatting start tags: each can add a level when the parser reopens it.
        formatting: usize,
        /// The tags that start a repair: formatting end tags and `a` and `nobr` start tags.
        repairs: usize,
        /// The start tags of the table family: each can imply two elements.
        tables: usize,
    }

    impl Generated {
        fn start(&mut self, name: &str) {
            self.markup.push_str(&open_tag(name));
            let formatting = matches!(name, "b" | "i" | "u" | "a" | "em" | "s" | "nobr");
            self.formatting += usize::from(formatting);
            self.repairs += usize::from(matches!(name, "a" | "nobr"));
            self.tables += usize::from(matches!(name, "table" | "tbody" | "tr" | "td"));
        }

        fn end(&mut self, name: &str) {
            self.markup.push_str(&close_tag(name));
            self.repairs +=
                usize::from(matches!(name, "b" | "i" | "u" | "a" | "em" | "s" | "nobr"));
        }

        fn repeat(&mut self, count: usize, name: &str) {
            (0..count).for_each(|_| self.start(name));
        }
    }

    fn generated(seed: u64) -> Generated {
        const NAMES: [&str; 17] = [
            "b", "i", "u", "a", "em", "span", "div", "p", "table", "tbody", "tr", "td", "select",
            "template", "svg", "li", "ul",
        ];
        const FORMATTING: [&str; 5] = ["b", "i", "u", "a", "em"];
        let mut state = seed.wrapping_mul(2_862_933_555_777_941_757).wrapping_add(3);
        let mut next = move |bound: usize| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) as usize % bound
        };
        let mut out = Generated {
            markup: String::new(),
            formatting: 0,
            repairs: 0,
            tables: 0,
        };
        out.repeat(500 + next(100), "div");
        for _ in 0..4 + next(8) {
            match next(7) {
                0 => {
                    let names: Vec<&str> = (0..=next(5)).map(|_| FORMATTING[next(5)]).collect();
                    for name in &names {
                        out.start(name);
                    }
                    out.start(["div", "p"][next(2)]);
                    out.markup.push('x');
                    out.end(names[0]);
                    out.repeat(50 + next(470), "span");
                }
                1 => {
                    for _ in 0..20 {
                        let name = NAMES[next(NAMES.len())];
                        match next(10) {
                            0..=5 => out.start(name),
                            6 => out.markup.push_str("x<!--c-->"),
                            _ => out.end(name),
                        }
                    }
                }
                2 => {
                    out.repeat(next(6), "tbody");
                    out.start("table");
                    out.start("td");
                    out.repeat(2, "b");
                    out.markup.push('x');
                }
                3 => {
                    out.start("p");
                    let count = next(40);
                    out.markup.push_str(&distinct_bolds(count));
                    out.formatting += count;
                    out.end("p");
                    out.markup.push('x');
                }
                4 => {
                    let unit = ["a", "nobr"][next(2)];
                    for _ in 0..=next(30) {
                        out.start(unit);
                        out.markup.push('1');
                        out.start("div");
                        out.markup.push('2');
                    }
                }
                5 => {
                    out.markup.push_str("<b id=1><i id=2>");
                    out.formatting += 2;
                    out.start("table");
                    out.markup.push('x');
                }
                _ => {
                    out.start("b");
                    for _ in 0..8 {
                        for name in ["i", "u", "s", "div"] {
                            out.start(name);
                        }
                    }
                    out.markup.push('x');
                    out.end("b");
                    out.repeat(next(100), "span");
                }
            }
        }
        out
    }

    fn parse_generated(seed: u64) -> (VirtualTree, ParsedHtml, Generated) {
        let case = generated(seed);
        let mut tree = VirtualTree::new();
        let context = node(&mut tree, "div");
        tree.hold(context).unwrap();
        let html = tree.parse_html(Some(context), &case.markup).unwrap();
        (tree, html, case)
    }

    #[test]
    fn generated_markup_keeps_the_tree_within_the_depth_bound() {
        for seed in 0..60 {
            let (_, html, case) = parse_generated(seed);

            // One level for each reopened formatting element, five for each of eight repairs, and
            // two implied elements for each table tag.
            let bound = 512 + case.formatting + 40 * case.repairs + 2 * case.tables;
            let depth = deepest_level(&html);
            assert!(
                depth <= bound,
                "seed {seed}, {} bytes: depth {depth} over {bound}",
                case.markup.len()
            );
        }
    }

    #[test]
    fn generated_markup_leaves_no_node_outside_the_result() {
        for seed in 0..60 {
            let (mut tree, html, case) = parse_generated(seed);

            let mut freed = tree.sweep();
            let mut listed: Vec<NodeId> = html.nodes.iter().map(|entry| entry.id).collect();
            listed.push(html.fragment);
            freed.sort_unstable();
            listed.sort_unstable();
            assert!(
                freed == listed,
                "seed {seed}, {} bytes: a node is outside the result",
                case.markup.len()
            );
        }
    }
}
