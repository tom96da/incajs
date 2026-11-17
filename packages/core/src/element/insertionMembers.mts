// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The members that insert nodes and texts in one host call.

import { insertNodes } from "../ffi/index.mts";
import { attributeText } from "./arguments.mts";
import {
  NON_DOCUMENT_CHILD_RULE,
  PARENT_NODE_RULE,
  entered,
  isDestroyed,
  isNode,
  left,
  nodeOf,
  receiverNode,
} from "./lifecycle.mts";
import type { InsertionPlace } from "../ffi/index.mts";
import type { HandleNode } from "./handleNode.mts";
import type { HandleBuilders, Receiver } from "./lifecycle.mts";
import type { ChildNode, Node, ParentNode, ParentNodeMutationMembers } from "./nodeTypes.mts";

/**
 * Inserts the arguments with one host call and keeps the node table in step with the result. A
 * member of `ChildNode` on a freed node returns before the host call.
 * @param rule - the receiver rule of the interface of the member
 * @param self - `this` of the member
 * @param place - where the nodes go
 * @param nodes - the arguments of the member
 * @param builders - the functions that build the handles of the new text nodes
 * @throws {TypeError} when `self` fails `rule`, an argument is a `Symbol`, or the
 *   host refuses the call
 * @throws {DOMException} when the host refuses the insertion
 */
function insertConverted(
  rule: Receiver,
  self: unknown,
  place: InsertionPlace,
  nodes: readonly unknown[],
  builders: HandleBuilders,
): void {
  const target = receiverNode(self, rule);
  const converted = nodes.map((value): HandleNode | string =>
    isNode(value) ? value : attributeText(value),
  );
  if (rule === NON_DOCUMENT_CHILD_RULE && isDestroyed(target)) return;
  const result = insertNodes(
    target.__node_id,
    place,
    converted.map((value) => (typeof value === "string" ? value : value.__node_id)),
  );
  if (result === null) return;
  const [textIds, removedIds] = result;
  const ids = textIds.values();
  for (const value of converted) {
    if (typeof value === "string") entered(builders.text(ids.next().value as number, value));
    else if (value.kind !== "fragment") entered(value);
  }
  for (const id of removedIds) {
    const handle = nodeOf(id);
    if (handle) left(handle);
  }
}

/**
 * Builds the members of `ParentNode` that insert nodes.
 * @param builders - the functions that build the handles of the new text nodes
 * @returns the members
 */
export function parentNodeMutationMembers(builders: HandleBuilders): ParentNodeMutationMembers {
  return {
    append(this: ParentNode, ...nodes: unknown[]): void {
      insertConverted(PARENT_NODE_RULE, this, "append", nodes, builders);
    },
    prepend(this: ParentNode, ...nodes: unknown[]): void {
      insertConverted(PARENT_NODE_RULE, this, "prepend", nodes, builders);
    },
    replaceChildren(this: ParentNode, ...nodes: unknown[]): void {
      insertConverted(PARENT_NODE_RULE, this, "replaceChildren", nodes, builders);
    },
  };
}

/**
 * Builds the members of `ChildNode`.
 * @param builders - the functions that build the handles of the new text nodes
 * @returns the members
 */
export function childNodeMutationMembers(builders: HandleBuilders): Omit<ChildNode, keyof Node> {
  return {
    before(this: Node, ...nodes: unknown[]): void {
      insertConverted(NON_DOCUMENT_CHILD_RULE, this, "before", nodes, builders);
    },
    after(this: Node, ...nodes: unknown[]): void {
      insertConverted(NON_DOCUMENT_CHILD_RULE, this, "after", nodes, builders);
    },
    replaceWith(this: Node, ...nodes: unknown[]): void {
      insertConverted(NON_DOCUMENT_CHILD_RULE, this, "replaceWith", nodes, builders);
    },
    remove(this: Node): void {
      insertConverted(NON_DOCUMENT_CHILD_RULE, this, "replaceWith", [], builders);
    },
  };
}
