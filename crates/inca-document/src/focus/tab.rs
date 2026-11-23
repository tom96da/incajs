// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Tab order: where a Tab press moves focus.

use inca_node::{NodeId, VirtualTree};
use inca_style::StyleCache;

use super::{FocusModel, FocusSurface, Start};

impl FocusModel {
    /// Moves focus one tab stop forward, or `backward`. With no node focused,
    /// the move starts from the last press or focus (see
    /// [`Self::press_start`]). Past the last stop nothing holds focus, and the
    /// next Tab starts over at the first. A move that finds no stop forgets
    /// where it started. The transition is reported by the next
    /// [`Self::apply_pending`]. Returns the node that took focus. The move
    /// also drops the scroll requests of earlier focus calls. From a node that no longer takes
    /// focus, the move goes to the nearest stop after its place in tree order (before it when
    /// `backward`), or forward with none after it to the first stop.
    pub fn tab_move(
        &mut self,
        tree: &VirtualTree,
        cache: &StyleCache,
        root: NodeId,
        surface: &mut impl FocusSurface,
        backward: bool,
    ) -> Option<NodeId> {
        let focused = self.held_node(surface.focused());
        let start = if focused.is_none() { self.start } else { None };
        let mut stops = Vec::new();
        let from = start.map(Start::node).or(focused);
        let mut from_at = None;
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = tree.get(id) else { continue };
            if from == Some(id) {
                from_at = Some(stops.len());
            }
            if self.tracked.contains(&id)
                && let Some(index) = Self::tab_index_of(tree, cache, id)
            {
                stops.push((id, index));
            }
            stack.extend(node.children().iter().rev());
        }
        let target = match (start, from.zip(from_at)) {
            (Some(Start::Press { .. }), Some((node, at))) => {
                after_press(&stops, node, at, backward)
            }
            (_, Some((node, at))) if stops.get(at).is_none_or(|stop| stop.0 != node) => {
                after_press(&stops, node, at, backward)
            }
            (Some(Start::Focus(node)), _) => next_stop(&stops, Some(node), backward),
            _ => next_stop(&stops, focused, backward),
        };
        if target.is_none() {
            self.start = None;
        }
        self.reveal.clear();
        self.retry.clear();
        if let Some(id) = target {
            surface.focus(id);
        } else {
            surface.blur();
        }
        target
    }
}

/// The target of a Tab press. `stops` lists every focusable node with its
/// `tabindex` in tree order. Positive values come first in ascending order,
/// then 0 in tree order, and negative values are outside the order. Past either
/// end the result is `None`. From a node outside the order the result is the
/// nearest stop after it (before it when `backward`) in tree order, or forward
/// with no stop after it the first stop. `current` `None` starts at the first
/// (or, `backward`, last) stop.
fn next_stop(stops: &[(NodeId, i32)], current: Option<NodeId>, backward: bool) -> Option<NodeId> {
    // (sort key, tree position, node)
    let mut ordered: Vec<((bool, i32), usize, NodeId)> = stops
        .iter()
        .enumerate()
        .filter(|(_, (_, index))| *index >= 0)
        // A single integer key gives 0 and the largest positive index the same key.
        .map(|(pos, &(id, index))| ((index == 0, index), pos, id))
        .collect();
    ordered.sort_by_key(|&(key, ..)| key);
    let at = current.and_then(|id| ordered.iter().position(|&(_, _, node)| node == id));
    let found = match (current, at) {
        (Some(_), Some(at)) => {
            if backward {
                at.checked_sub(1).and_then(|i| ordered.get(i))
            } else {
                ordered.get(at + 1)
            }
        }
        (Some(id), None) => match stops.iter().position(|&(node, _)| node == id) {
            Some(here) if backward => ordered.iter().filter(|s| s.1 < here).max_by_key(|s| s.1),
            Some(here) => ordered
                .iter()
                .filter(|s| s.1 > here)
                .min_by_key(|s| s.1)
                .or_else(|| ordered.first()),
            None if backward => ordered.last(),
            None => ordered.first(),
        },
        (None, _) if backward => ordered.last(),
        (None, _) => ordered.first(),
    };
    found.map(|&(_, _, id)| id)
}

/// The target of a Tab press from `node`, which sits at `at` among `stops` in tree order and is a
/// pressed node or a node outside the order. Forward, it is the first stop after `node`, or with
/// none after it the first stop of the order when `node` is outside the order. Backward, it is the
/// last stop up to `node` itself.
fn after_press(stops: &[(NodeId, i32)], node: NodeId, at: usize, backward: bool) -> Option<NodeId> {
    let own = stops.get(at).filter(|&&(id, _)| id == node);
    let split = at + usize::from(own.is_some());
    let in_order = |stop: &&(NodeId, i32)| stop.1 >= 0;
    if backward {
        return stops[..split]
            .iter()
            .rev()
            .find(in_order)
            .map(|&(id, _)| id);
    }
    match stops[split..].iter().find(in_order) {
        Some(&(id, _)) => Some(id),
        None if own.is_some_and(|&(_, index)| index >= 0) => None,
        None => next_stop(stops, None, false),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::{FakeSurface, ROOT, rooted_tree};
    use rstest::rstest;

    // Nodes 1 to 6 have tabindex 3, 1, 0, 0, -1 and the largest value.
    const STOPS: [(NodeId, i32); 6] = [(1, 3), (2, 1), (3, 0), (4, 0), (5, -1), (6, i32::MAX)];

    fn walk(stops: &[(NodeId, i32)], start: Option<NodeId>, backward: bool) -> Vec<Option<NodeId>> {
        let mut current = start;
        let mut moves = Vec::new();
        for _ in 0..stops.len() + 2 {
            let target = next_stop(stops, current, backward);
            moves.push(target);
            current = target;
        }
        moves
    }

    #[test]
    fn next_stop_orders_positive_values_then_zeros_in_tree_order() {
        assert_eq!(
            walk(&STOPS, None, false),
            [
                Some(2),
                Some(1),
                Some(6),
                Some(3),
                Some(4),
                None,
                Some(2),
                Some(1)
            ]
        );
        assert_eq!(
            walk(&STOPS, None, true),
            [
                Some(4),
                Some(3),
                Some(6),
                Some(1),
                Some(2),
                None,
                Some(4),
                Some(3)
            ]
        );
    }

    #[test]
    fn next_stop_from_a_negative_stop_continues_in_tree_order() {
        assert_eq!(next_stop(&STOPS, Some(5), false), Some(6));
        assert_eq!(next_stop(&STOPS, Some(5), true), Some(4));
        let stops = [(1, 3), (2, -1), (3, 0), (4, 1)];
        assert_eq!(next_stop(&stops, Some(2), false), Some(3));
        assert_eq!(next_stop(&stops, Some(2), true), Some(1));
        let stops = [(1, 1), (2, 0), (3, -1)];
        assert_eq!(next_stop(&stops, Some(3), false), Some(1));
        let stops = [(1, -1), (2, 1)];
        assert_eq!(next_stop(&stops, Some(1), true), None);
    }

    #[test]
    fn next_stop_from_an_unknown_node_starts_at_an_end() {
        assert_eq!(next_stop(&STOPS, Some(99), false), Some(2));
        assert_eq!(next_stop(&STOPS, Some(99), true), Some(4));
    }

    #[rstest]
    #[case::forward(false)]
    #[case::backward(true)]
    fn next_stop_is_none_without_a_reachable_stop(#[case] backward: bool) {
        assert_eq!(next_stop(&[], None, backward), None);
        assert_eq!(next_stop(&[(1, -1)], None, backward), None);
    }

    /// Three tab stops under the root, all with a handle, and a model that applied them.
    fn three_stops() -> (VirtualTree, [NodeId; 3], FocusModel, FakeSurface) {
        let (tree, ids) = rooted_tree(3);
        let mut model = FocusModel::default();
        let mut surface = FakeSurface::default();
        for &id in &ids {
            model.mark_tab_dirty(id);
        }
        let _ = model.apply_pending(&tree, &StyleCache::new(), &mut surface);
        (tree, [ids[0], ids[1], ids[2]], model, surface)
    }

    fn tab(
        (tree, _, model, surface): &mut (VirtualTree, [NodeId; 3], FocusModel, FakeSurface),
        backward: bool,
    ) -> Option<NodeId> {
        model.tab_move(tree, &StyleCache::new(), ROOT, surface, backward)
    }

    #[rstest]
    #[case::forward(false, 0)]
    #[case::backward(true, 2)]
    fn tab_from_no_focus_takes_the_first_or_last_stop(#[case] backward: bool, #[case] stop: usize) {
        let mut stops = three_stops();
        let expected = stops.1[stop];

        assert_eq!(tab(&mut stops, backward), Some(expected));
        assert_eq!(stops.3.focus, Some(expected));
    }

    #[rstest]
    #[case::forward(false, 2)]
    #[case::backward(true, 0)]
    fn tab_from_a_focused_stop_takes_its_neighbour(#[case] backward: bool, #[case] stop: usize) {
        let mut stops = three_stops();
        stops.3.focus = Some(stops.1[1]);
        let expected = stops.1[stop];

        assert_eq!(tab(&mut stops, backward), Some(expected));
    }

    #[rstest]
    #[case::forward_takes_the_next_stop(false, 2)]
    #[case::backward_takes_the_pressed_stop(true, 1)]
    fn tab_after_a_press_on_a_stop_continues_from_it(#[case] backward: bool, #[case] stop: usize) {
        let mut stops = three_stops();
        let pressed = stops.1[1];
        stops.2.press_start(pressed, ROOT, None);
        let expected = stops.1[stop];

        assert_eq!(tab(&mut stops, backward), Some(expected));
    }

    #[test]
    fn tab_past_the_last_stop_blurs_and_forgets_where_tab_started() {
        let mut stops = three_stops();
        stops.3.focus = Some(stops.1[2]);
        stops.2.press_start(stops.1[2], ROOT, None);

        assert_eq!(tab(&mut stops, false), None);
        assert_eq!(stops.3.focus, None);
        assert_eq!(stops.2.start_node(), None);
    }

    #[test]
    fn tab_skips_a_stop_without_a_handle() {
        let mut stops = three_stops();
        stops.3.handles.remove(&stops.1[0]);
        stops.2.tracked.remove(&stops.1[0]);
        let expected = stops.1[1];

        assert_eq!(tab(&mut stops, false), Some(expected));
    }

    #[test]
    fn a_tab_move_drops_the_scroll_requests_of_earlier_focus_calls() {
        let mut stops = three_stops();
        stops.2.request_focus(stops.1[0], true);
        let _ = stops
            .2
            .apply_pending(&stops.0, &StyleCache::new(), &mut stops.3);

        let _ = tab(&mut stops, false);

        assert_eq!(stops.2.take_reveals(), (vec![], vec![]));
    }
}
