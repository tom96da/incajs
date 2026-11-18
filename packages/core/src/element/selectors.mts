// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The selector members: `matches()`, `closest()` and the queries of an element and the document.

import { closestNode, queryNode, queryNodes } from "../ffi/index.mts";
import { attributeText, requireArguments } from "./arguments.mts";
import { nodeListOf } from "./collections.mts";
import { ELEMENT_RULE, PARENT_NODE_RULE, receiverNode } from "./lifecycle.mts";
import type { NodeId } from "../types.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type { Element } from "./types.mts";

/** The members `querySelector()` and `querySelectorAll()` of an element and of the document. */
export type QueryMembers = Pick<Element, "querySelector" | "querySelectorAll">;

/** The members {@link selectorMembers} builds. */
export type SelectorMembers = Pick<Element, "closest">;

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
 * Finds the element handle of an element node the host just returned.
 * @param id - the native node's id
 * @param builders - the lookup of a handle by node id
 * @returns the handle
 */
function elementHandle(id: NodeId, builders: HandleBuilders): Element {
  return builders.node(id) as Element;
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
      const found = queryNode(node.__node_id, text);
      return found === null ? null : elementHandle(found, builders);
    },
    querySelectorAll(selectors: string) {
      const node = receiverNode(this, PARENT_NODE_RULE);
      const text = selectorText(arguments, "querySelectorAll", selectors);
      const found = queryNodes(node.__node_id, text).map((nodeId) =>
        elementHandle(nodeId, builders),
      );
      return nodeListOf(() => found);
    },
  };
}

/**
 * Builds `closest()` of `Element`. A node the host freed finds nothing, and a bad selector still
 * throws.
 * @param builders - the lookup of a handle by node id
 * @returns the members
 */
export function selectorMembers(builders: HandleBuilders): SelectorMembers {
  return {
    closest(selectors: string) {
      const element = receiverNode(this, ELEMENT_RULE);
      const text = selectorText(arguments, "closest", selectors);
      const found = closestNode(element.__node_id, text);
      return found === null ? null : elementHandle(found, builders);
    },
  };
}
