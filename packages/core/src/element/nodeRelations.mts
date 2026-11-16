// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The relation members of every handle. The host compares the nodes of a live tree.
// A freed node is in a tree of its own, so its answers come from the handle.

import { compareNodePosition, isEqualNode, rootOf } from "../ffi/index.mts";
import { nodeArgument, requireArguments } from "./arguments.mts";
import { isDestroyed, NODE_RULE, receiverNode } from "./lifecycle.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type { NodeMembers } from "./nodeTypes.mts";
import type { IncaNode } from "./types.mts";

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
 * Builds the relation members of `Node`.
 * @param builders - the lookup of a handle by node id
 * @returns the members
 */
export function nodeRelationMembers(builders: HandleBuilders): NodeRelationMembers {
  const nullable = (value: unknown): IncaNode | null =>
    value === null || value === undefined ? null : nodeArgument(value, 1);
  return {
    contains(other: unknown) {
      const node = receiverNode(this, NODE_RULE);
      const found = nullable(other);
      if (found === null) return false;
      if (found === node) return true;
      if (isDestroyed(node) || isDestroyed(found)) return false;
      return (compareNodePosition(node.__node_id, found.__node_id) & CONTAINED_BY) !== 0;
    },
    getRootNode() {
      const node = receiverNode(this, NODE_RULE);
      return isDestroyed(node) ? node : (builders.node(rootOf(node.__node_id)) ?? node);
    },
    compareDocumentPosition(other: IncaNode) {
      const node = receiverNode(this, NODE_RULE);
      requireArguments(arguments, 1, "compareDocumentPosition");
      const found = nodeArgument(other, 1);
      if (found === node) return 0;
      if (isDestroyed(node) || isDestroyed(found)) {
        return DISCONNECTED | (found.__node_id < node.__node_id ? PRECEDING : FOLLOWING);
      }
      return compareNodePosition(node.__node_id, found.__node_id);
    },
    isSameNode(other: unknown) {
      return nullable(other) === receiverNode(this, NODE_RULE);
    },
    isEqualNode(other: unknown) {
      const node = receiverNode(this, NODE_RULE);
      const found = nullable(other);
      if (found === null) return false;
      if (isDestroyed(node) || isDestroyed(found)) return found === node;
      return isEqualNode(node.__node_id, found.__node_id);
    },
  } as NodeRelationMembers;
}
