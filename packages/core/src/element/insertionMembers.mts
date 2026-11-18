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
  leftSubtrees,
  receiverNode,
} from "./lifecycle.mts";
import type { InsertionPlace } from "../ffi/index.mts";
import type { HandleNode } from "./handleNode.mts";
import type { Receiver } from "./lifecycle.mts";
import type { ChildNode, Node, ParentNode, ParentNodeMutationMembers } from "./nodeTypes.mts";

/**
 * Inserts the arguments with one host call and keeps the node table in step with the result. A
 * member of `ChildNode` on a freed node returns before the host call.
 * @param rule - the receiver rule of the interface of the member
 * @param self - `this` of the member
 * @param place - where the nodes go
 * @param nodes - the arguments of the member
 * @throws {TypeError} when `self` fails `rule`, an argument is a `Symbol`, or the
 *   host refuses the call
 * @throws {DOMException} when the host refuses the insertion
 */
function insertConverted(
  rule: Receiver,
  self: unknown,
  place: InsertionPlace,
  nodes: readonly unknown[],
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
  const [, removed] = result;
  for (const value of converted) {
    if (typeof value !== "string" && value.kind !== "fragment") entered(value);
  }
  leftSubtrees(removed);
}

/**
 * Builds the members of `ParentNode` that insert nodes.
 * @returns the members
 */
export function parentNodeMutationMembers(): ParentNodeMutationMembers {
  return {
    append(this: ParentNode, ...nodes: unknown[]): void {
      insertConverted(PARENT_NODE_RULE, this, "append", nodes);
    },
    prepend(this: ParentNode, ...nodes: unknown[]): void {
      insertConverted(PARENT_NODE_RULE, this, "prepend", nodes);
    },
    replaceChildren(this: ParentNode, ...nodes: unknown[]): void {
      insertConverted(PARENT_NODE_RULE, this, "replaceChildren", nodes);
    },
  };
}

/**
 * Builds the members of `ChildNode`.
 * @returns the members
 */
export function childNodeMutationMembers(): Omit<ChildNode, keyof Node> {
  return {
    before(this: Node, ...nodes: unknown[]): void {
      insertConverted(NON_DOCUMENT_CHILD_RULE, this, "before", nodes);
    },
    after(this: Node, ...nodes: unknown[]): void {
      insertConverted(NON_DOCUMENT_CHILD_RULE, this, "after", nodes);
    },
    replaceWith(this: Node, ...nodes: unknown[]): void {
      insertConverted(NON_DOCUMENT_CHILD_RULE, this, "replaceWith", nodes);
    },
    remove(this: Node): void {
      insertConverted(NON_DOCUMENT_CHILD_RULE, this, "replaceWith", []);
    },
  };
}
