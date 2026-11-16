// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { childNodeIds, linkedNode } from "../ffi/index.mts";
import { htmlCollectionOf } from "./collections.mts";
import { isDestroyed } from "./lifecycle.mts";
import type { NodeLink } from "../ffi/index.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type { IncaComment, IncaElement } from "./types.mts";

/** The element members of the tree: text leaves and comments count as none. */
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

/**
 * Builds the element tree members of `element`. Each read asks the host tree again. A freed
 * element has no links, and a node with no handle is skipped.
 * @param element - the handle the members belong to
 * @param builders - the lookup of a handle by node id
 * @returns the members, as getters
 */
export function elementTreeMembers(
  element: IncaElement | IncaComment,
  builders: HandleBuilders,
): ElementTreeMembers {
  const id = element.__node_id;
  const handleOf = (nodeId: number): IncaElement | undefined =>
    builders.node(nodeId) as IncaElement | undefined;
  const reach = (link: NodeLink): IncaElement | null => {
    if (isDestroyed(element)) return null;
    const found = linkedNode(id, link, true);
    return found === null ? null : (handleOf(found) ?? null);
  };
  const liveChildren = (): IncaElement[] =>
    isDestroyed(element) ? [] : childNodeIds(id, true).flatMap((child) => handleOf(child) ?? []);
  let children: IncaElement["children"] | undefined;
  return {
    get children() {
      return (children ??= htmlCollectionOf(liveChildren));
    },
    get parentElement() {
      return reach("parent");
    },
    get firstElementChild() {
      return reach("first");
    },
    get lastElementChild() {
      return reach("last");
    },
    get childElementCount() {
      return liveChildren().length;
    },
    get previousElementSibling() {
      return reach("previous");
    },
    get nextElementSibling() {
      return reach("next");
    },
  };
}
