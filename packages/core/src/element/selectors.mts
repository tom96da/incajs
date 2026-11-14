// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The selector members: `matches()`, `closest()` and the queries of an element and the document.

import { checkSelector, closestNode, matchesNode, queryNode, queryNodes } from "../ffi/index.mts";
import { attributeText, requireArguments } from "./arguments.mts";
import { nodeListOf } from "./collections.mts";
import { isDestroyed } from "./lifecycle.mts";
import type { IncaDocument, NodeId } from "../types.mts";
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
 * @returns the selector text
 */
function selectorText(args: readonly unknown[], method: string): string {
  requireArguments(args, 1, method);
  return attributeText(args[0]);
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
 * Builds the query members of a node that holds elements below it.
 * @param node - the element or the document the members belong to
 * @param builders - the lookup of a handle by node id
 * @returns the members
 */
export function queryMembers(
  node: IncaElement | IncaDocument,
  builders: HandleBuilders,
): QueryMembers {
  const id = node.__node_id;
  return {
    querySelector: (...args: [selectors: string]) => {
      const text = selectorText(args, "querySelector");
      if (isDestroyed(node)) {
        checkSelector(text);
        return null;
      }
      const found = queryNode(id, text);
      return found === null ? null : (elementHandle(found, builders) ?? null);
    },
    querySelectorAll: (...args: [selectors: string]) => {
      const text = selectorText(args, "querySelectorAll");
      if (isDestroyed(node)) {
        checkSelector(text);
        return nodeListOf(() => []);
      }
      const found = queryNodes(id, text).flatMap((nodeId) => elementHandle(nodeId, builders) ?? []);
      return nodeListOf(() => found);
    },
  };
}

/**
 * Builds `matches()` and `closest()` of an element.
 * @param element - the handle the members belong to
 * @param builders - the lookup of a handle by node id
 * @returns the members
 */
export function selectorMembers(element: IncaElement, builders: HandleBuilders): SelectorMembers {
  const id = element.__node_id;
  return {
    matches: (...args: [selectors: string]) => {
      const text = selectorText(args, "matches");
      if (isDestroyed(element)) {
        checkSelector(text);
        return false;
      }
      return matchesNode(id, text);
    },
    closest: (...args: [selectors: string]) => {
      const text = selectorText(args, "closest");
      if (isDestroyed(element)) {
        checkSelector(text);
        return null;
      }
      const found = closestNode(id, text);
      return found === null ? null : (elementHandle(found, builders) ?? null);
    },
  };
}
