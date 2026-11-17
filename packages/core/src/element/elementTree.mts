// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { childNodeIds, linkedNode } from "../ffi/index.mts";
import { htmlCollectionOf } from "./collections.mts";
import {
  isDestroyed,
  receiverOf,
  NON_DOCUMENT_CHILD_RULE,
  PARENT_NODE_RULE,
  receiverNode,
} from "./lifecycle.mts";
import type { NodeLink } from "../ffi/index.mts";
import type { HandleNode } from "./handleNode.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type {
  Node,
  NonDocumentTypeChildNode,
  ParentNode,
  ParentNodeMutationMembers,
} from "./nodeTypes.mts";
import type { Element } from "./types.mts";

/** The ParentNode members the tree gives, the queries excepted. Comments count as no element. */
export type ElementTreeMembers = Omit<
  ParentNode,
  keyof Node | keyof ParentNodeMutationMembers | "querySelector" | "querySelectorAll"
>;

/**
 * Finds the element handle of a node id.
 * @param builders - the lookup of a handle by node id
 * @param id - the native node's id
 * @returns the handle, or `undefined` when the node has none
 */
function elementHandle(builders: HandleBuilders, id: number): Element | undefined {
  return builders.node(id) as Element | undefined;
}

/**
 * Builds the ParentNode members of the tree, for the interfaces that hold children. Each read asks
 * the host tree again. A freed node has no links, and a node with no handle is skipped.
 * @param builders - the lookup of a handle by node id
 * @returns the members, as getters
 */
export function parentNodeMembers(builders: HandleBuilders): ElementTreeMembers {
  const reach = (self: unknown, link: NodeLink): Element | null => {
    const node = receiverNode(self, PARENT_NODE_RULE);
    if (isDestroyed(node)) return null;
    const found = linkedNode(node.__node_id, link, true);
    return found === null ? null : (elementHandle(builders, found) ?? null);
  };
  const liveChildren = (node: HandleNode): Element[] =>
    isDestroyed(node)
      ? []
      : childNodeIds(node.__node_id, true).flatMap((child) => elementHandle(builders, child) ?? []);
  return {
    get children() {
      const state = receiverOf(this, PARENT_NODE_RULE);
      const node = this as unknown as HandleNode;
      return (state.children ??= htmlCollectionOf(() => liveChildren(node)));
    },
    get firstElementChild() {
      return reach(this, "first");
    },
    get lastElementChild() {
      return reach(this, "last");
    },
    get childElementCount() {
      return liveChildren(receiverNode(this, PARENT_NODE_RULE)).length;
    },
  };
}

/**
 * Builds the sibling members of the nodes that have a parent and are no document type.
 * @param builders - the lookup of a handle by node id
 * @returns the members, as getters
 */
export function nonDocumentTypeChildNodeMembers(
  builders: HandleBuilders,
): Omit<NonDocumentTypeChildNode, keyof Node> {
  const reach = (self: unknown, link: NodeLink): Element | null => {
    const node = receiverNode(self, NON_DOCUMENT_CHILD_RULE);
    if (isDestroyed(node)) return null;
    const found = linkedNode(node.__node_id, link, true);
    return found === null ? null : (elementHandle(builders, found) ?? null);
  };
  return {
    get previousElementSibling() {
      return reach(this, "previous");
    },
    get nextElementSibling() {
      return reach(this, "next");
    },
  };
}
