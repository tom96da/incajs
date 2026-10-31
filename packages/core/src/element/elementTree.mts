// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { IncaElement, IncaNode } from "./types.mts";

/** The element members of the tree: a `<text>` element is text content and counts as none. */
export type ElementTreeMembers = Pick<
  IncaElement,
  | "parentElement"
  | "firstElementChild"
  | "lastElementChild"
  | "childElementCount"
  | "previousElementSibling"
  | "nextElementSibling"
>;

function isElementNode(node: IncaNode): node is IncaElement {
  return node.kind === "element" && node.__tag !== "text";
}

/**
 * Lists the children of `element` that are elements.
 * @param element - the parent
 * @returns the element children in order
 */
function elementChildren(element: IncaElement): IncaElement[] {
  return element.children.filter(isElementNode);
}

function siblingElement(element: IncaElement, step: 1 | -1): IncaElement | null {
  const siblings = element.parent?.children ?? [];
  for (let at = siblings.indexOf(element) + step; at >= 0 && at < siblings.length; at += step) {
    const sibling = siblings[at]!;
    if (isElementNode(sibling)) return sibling;
  }
  return null;
}

/**
 * Builds the element tree members of `element`. Each read looks at the shadow tree again.
 * @param element - the handle the members belong to
 * @returns the members, as getters
 */
export function elementTreeMembers(element: IncaElement): ElementTreeMembers {
  return {
    get parentElement() {
      return element.parent;
    },
    get firstElementChild() {
      return elementChildren(element)[0] ?? null;
    },
    get lastElementChild() {
      return elementChildren(element).at(-1) ?? null;
    },
    get childElementCount() {
      return elementChildren(element).length;
    },
    get previousElementSibling() {
      return siblingElement(element, -1);
    },
    get nextElementSibling() {
      return siblingElement(element, 1);
    },
  };
}
