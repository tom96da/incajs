// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The next stop of a Tab press.

use inca_node::NodeId;

/// The target of a Tab press. `stops` lists every focusable node with its
/// `tabindex` in tree order. Positive values come first in ascending order,
/// then 0 in tree order, and negative values are outside the order. Past either
/// end the result is `None`. From a node outside the order the result is the
/// nearest stop after it (before it when `backward`) in tree order, or forward
/// with no stop after it the first stop. `current` `None` starts at the first
/// (or, `backward`, last) stop.
pub(super) fn next_stop(
    stops: &[(NodeId, i32)],
    current: Option<NodeId>,
    backward: bool,
) -> Option<NodeId> {
    // (sort key, tree position, node)
    let mut ordered: Vec<(i32, usize, NodeId)> = stops
        .iter()
        .enumerate()
        .filter(|(_, (_, index))| *index >= 0)
        .map(|(pos, &(id, index))| (if index > 0 { index } else { i32::MAX }, pos, id))
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
pub(super) fn after_press(
    stops: &[(NodeId, i32)],
    node: NodeId,
    at: usize,
    backward: bool,
) -> Option<NodeId> {
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
    use rstest::rstest;

    // Nodes 1 to 6 have tabindex 3, 1, 0, 0, -1 and 2.
    const STOPS: [(NodeId, i32); 6] = [(1, 3), (2, 1), (3, 0), (4, 0), (5, -1), (6, 2)];

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
                Some(6),
                Some(1),
                Some(3),
                Some(4),
                None,
                Some(2),
                Some(6)
            ]
        );
        assert_eq!(
            walk(&STOPS, None, true),
            [
                Some(4),
                Some(3),
                Some(1),
                Some(6),
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
}
