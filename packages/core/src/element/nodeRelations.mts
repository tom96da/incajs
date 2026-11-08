// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The relation members of every handle. The host compares the nodes of a live tree.
// A freed node is in a tree of its own, so its answers come from the handle.

import { compareNodePosition, isEqualNode, rootOf } from "../tree.mts";
import { requireArguments } from "./attributes.mts";
import { isDestroyed } from "./lifecycle.mts";
import { nodeArgument } from "./nodeMutation.mts";
import type { HandleBuilders } from "./nodeMutation.mts";
import type { IncaNode, NodeMembers } from "./types.mts";

/** The members {@link nodeRelationMembers} builds. */
export type NodeRelationMembers = Pick<
  NodeMembers,
  "contains" | "getRootNode" | "compareDocumentPosition" | "isSameNode" | "isEqualNode"
>;

/** The bit of `compareDocumentPosition` for a node that the other contains. */
const CONTAINED_BY = 16;

/** The bits of `compareDocumentPosition` for nodes in different trees. */
const DISCONNECTED = 1 | 32;
const PRECEDING = 2;
const FOLLOWING = 4;

/**
 * Builds the relation members of `node`.
 * @param node - the handle the members belong to
 * @param builders - the lookup of a handle by node id
 * @returns the members
 */
export function nodeRelationMembers(node: IncaNode, builders: HandleBuilders): NodeRelationMembers {
  const id = node.__node_id;
  const nullable = (value: unknown): IncaNode | null =>
    value === null || value === undefined ? null : nodeArgument(value, 1);
  return {
    contains: (other: unknown) => {
      const found = nullable(other);
      if (found === null) return false;
      if (found === node) return true;
      if (isDestroyed(node) || isDestroyed(found)) return false;
      return (compareNodePosition(id, found.__node_id) & CONTAINED_BY) !== 0;
    },
    getRootNode: () => {
      return isDestroyed(node) ? node : builders.node(rootOf(id))!;
    },
    compareDocumentPosition: (...args: [other: IncaNode]) => {
      requireArguments(args, 1, "compareDocumentPosition");
      const found = nodeArgument(args[0], 1);
      if (found === node) return 0;
      if (isDestroyed(node) || isDestroyed(found)) {
        return DISCONNECTED | (found.__node_id < id ? PRECEDING : FOLLOWING);
      }
      return compareNodePosition(id, found.__node_id);
    },
    isSameNode: (other: unknown) => nullable(other) === node,
    isEqualNode: (other: unknown) => {
      const found = nullable(other);
      if (found === null) return false;
      if (isDestroyed(node) || isDestroyed(found)) return found === node;
      return isEqualNode(id, found.__node_id);
    },
  } as NodeRelationMembers;
}
