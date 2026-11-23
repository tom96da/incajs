// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The sink of the style warnings and the record of the lines each node has printed.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use inca_node::{NodeId, VirtualNode, VirtualTree};

/// Receives the style warning lines. A node prints each line once. The sink writes to stderr by
/// default.
pub(crate) struct StyleWarnings {
    sink: Rc<dyn Fn(&str)>,
    /// The lines printed for each node, as the text without the node prefix.
    printed: HashMap<NodeId, HashSet<String>>,
}

impl Default for StyleWarnings {
    fn default() -> Self {
        Self {
            sink: Rc::new(|line| eprintln!("{line}")),
            printed: HashMap::new(),
        }
    }
}

impl StyleWarnings {
    /// Replaces the sink that receives the lines.
    pub(crate) fn set_sink(&mut self, sink: Rc<dyn Fn(&str)>) {
        self.sink = sink;
    }

    /// Prints the `warning` of a `<style>` sheet as `<style>: <warning>`.
    pub(crate) fn report_sheet(&self, warning: &str) {
        (self.sink)(&format!("<style>: {warning}"));
    }

    /// Prints the `warning` of a style of `node_id` as `node <id> (<tag>): <warning>`, once for
    /// each node and line.
    pub(crate) fn report(&mut self, tree: &VirtualTree, node_id: NodeId, warning: &str) {
        if self
            .printed
            .entry(node_id)
            .or_default()
            .insert(warning.to_owned())
        {
            let tag = tree.get(node_id).map_or("", VirtualNode::local_name);
            (self.sink)(&format!("node {node_id} ({tag}): {warning}"));
        }
    }

    /// Drops the lines recorded for a freed node.
    pub(crate) fn forget(&mut self, node_id: NodeId) {
        self.printed.remove(&node_id);
    }

    /// Whether no node has a recorded line.
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.printed.is_empty()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::cell::RefCell;

    use super::*;

    /// A `StyleWarnings` that sends each line to a list the caller reads.
    fn collecting() -> (StyleWarnings, Rc<RefCell<Vec<String>>>) {
        let lines = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&lines);
        let mut warnings = StyleWarnings::default();
        warnings.set_sink(Rc::new(move |line| sink.borrow_mut().push(line.to_owned())));
        (warnings, lines)
    }

    #[test]
    fn a_line_prints_once_for_each_node() {
        let mut tree = VirtualTree::new();
        let a = tree.create_node("div").unwrap();
        let b = tree.create_node("p").unwrap();
        let (mut warnings, lines) = collecting();

        warnings.report(&tree, a, "x");
        warnings.report(&tree, a, "x");
        warnings.report(&tree, b, "x");

        assert_eq!(
            *lines.borrow(),
            [format!("node {a} (div): x"), format!("node {b} (p): x")]
        );
    }

    #[test]
    fn a_forgotten_node_prints_its_line_again() {
        let mut tree = VirtualTree::new();
        let a = tree.create_node("div").unwrap();
        let (mut warnings, lines) = collecting();
        warnings.report(&tree, a, "x");

        warnings.forget(a);
        assert!(warnings.is_empty());
        warnings.report(&tree, a, "x");

        assert_eq!(lines.borrow().len(), 2);
    }

    #[test]
    fn a_sheet_line_prints_with_the_style_prefix() {
        let (warnings, lines) = collecting();

        warnings.report_sheet("y");

        assert_eq!(*lines.borrow(), ["<style>: y"]);
    }
}
