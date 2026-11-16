// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The selector members: `matches()`, `closest()` and the queries of an element and the document.

import { checkSelector, closestNode, matchesNode, queryNode, queryNodes } from "../ffi/index.mts";
import { attributeText, requireArguments } from "./arguments.mts";
import { nodeListOf } from "./collections.mts";
import { isDestroyed, ELEMENT_RULE, PARENT_NODE_RULE, receiverNode } from "./lifecycle.mts";
import type { NodeId } from "../types.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type { IncaElement } from "./types.mts";

/** The members `querySelector()` and `querySelectorAll()` of an element and of the document. */
export type QueryMembers = Pick<IncaElement, "querySelector" | "querySelectorAll">;

/** The members {@link selectorMembers} builds. */
export type SelectorMembers = Pick<IncaElement, "matches" | "closest">;

/**
 * Checks the argument count of a selector method and converts the selector to text.
 * @param args - the arguments the call received
 * @param method - the method name for the message
 * @param selectors - the first argument
 * @returns the selector text
 */
function selectorText(args: ArrayLike<unknown>, method: string, selectors: unknown): string {
  requireArguments(args, 1, method);
  return attributeText(selectors);
}

/**
 * Finds the element handle of an element node.
 * @param id - the native node's id
 * @param builders - the lookup of a handle by node id
 * @returns the handle, or `undefined` when the node has none
 */
function elementHandle(id: NodeId, builders: HandleBuilders): IncaElement | undefined {
  return builders.node(id) as IncaElement | undefined;
}

/**
 * Builds the query members of the interfaces that hold elements below them.
 * @param builders - the lookup of a handle by node id
 * @returns the members
 */
export function queryMembers(builders: HandleBuilders): QueryMembers {
  return {
    querySelector(selectors: string) {
      const node = receiverNode(this, PARENT_NODE_RULE);
      const text = selectorText(arguments, "querySelector", selectors);
      if (isDestroyed(node)) {
        checkSelector(text);
        return null;
      }
      const found = queryNode(node.__node_id, text);
      return found === null ? null : (elementHandle(found, builders) ?? null);
    },
    querySelectorAll(selectors: string) {
      const node = receiverNode(this, PARENT_NODE_RULE);
      const text = selectorText(arguments, "querySelectorAll", selectors);
      if (isDestroyed(node)) {
        checkSelector(text);
        return nodeListOf(() => []);
      }
      const found = queryNodes(node.__node_id, text).flatMap(
        (nodeId) => elementHandle(nodeId, builders) ?? [],
      );
      return nodeListOf(() => found);
    },
  };
}

/**
 * Builds `matches()` and `closest()` of `Element`.
 * @param builders - the lookup of a handle by node id
 * @returns the members
 */
export function selectorMembers(builders: HandleBuilders): SelectorMembers {
  return {
    matches(selectors: string) {
      const element = receiverNode(this, ELEMENT_RULE);
      const text = selectorText(arguments, "matches", selectors);
      if (isDestroyed(element)) {
        checkSelector(text);
        return false;
      }
      return matchesNode(element.__node_id, text);
    },
    closest(selectors: string) {
      const element = receiverNode(this, ELEMENT_RULE);
      const text = selectorText(arguments, "closest", selectors);
      if (isDestroyed(element)) {
        checkSelector(text);
        return null;
      }
      const found = closestNode(element.__node_id, text);
      return found === null ? null : (elementHandle(found, builders) ?? null);
    },
  };
}
