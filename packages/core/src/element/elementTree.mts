// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { htmlCollectionOf } from "./collections.mts";
import type { IncaElement, IncaNode } from "./types.mts";

/** The element members of the tree: text leaves and comment stand-ins count as none. */
export type ElementTreeMembers = Pick<
  IncaElement,
  | "children"
  | "parentElement"
  | "firstElementChild"
  | "lastElementChild"
  | "childElementCount"
  | "previousElementSibling"
  | "nextElementSibling"
>;

function isElementNode(node: IncaNode): node is IncaElement {
  return node.kind === "element";
}

/**
 * Lists the entries of `element.__children` that are elements.
 * @param element - the parent
 * @returns the element children in order
 */
function elementChildren(element: IncaElement): IncaElement[] {
  return element.__children.filter(isElementNode);
}

function siblingElement(element: IncaElement, step: 1 | -1): IncaElement | null {
  const siblings = element.parent?.__children ?? [];
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
  let children: IncaElement["children"] | undefined;
  return {
    get children() {
      return (children ??= htmlCollectionOf(() => elementChildren(element)));
    },
    get parentElement() {
      return element.parent?.kind === "element" ? element.parent : null;
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
