// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The copy of a node: the host copies the nodes and the handles follow its pre-order.

import { cloneNode as cloneHostNode, nameOf } from "../ffi/index.mts";
import { existingContent } from "./lifecycle.mts";
import { keep, link } from "./shadowTree.mts";
import type { IncaDocument } from "../types.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type { IncaElement, IncaNode, IncaParent } from "./types.mts";

/**
 * Copies a node into handles. The host copies the nodes and the handles follow its pre-order.
 * @param node - the node to copy
 * @param deep - whether the descendants are copied too
 * @param builders - the functions that build the handles
 * @returns the held, parentless copy
 */
export function cloneNode<T extends IncaNode>(node: T, deep: boolean, builders: HandleBuilders): T {
  const copies = cloneHostNode(node.__node_id, deep);
  let next = 0;
  // The document has no clone and no parent, so no copy walk reaches it.
  type Copyable = Exclude<IncaNode, IncaDocument>;
  const build = (original: Copyable): IncaNode => {
    const entry = copies[next++]!;
    const id = entry[0];
    if (original.kind === "text") return builders.text(id, original.text);
    const copy =
      original.kind === "fragment"
        ? builders.fragment!(id)
        : builders.element!(
            id,
            original.kind,
            original.kind === "element" ? nameOf(entry) : undefined,
          );
    if (original.kind === "element" && existingContent(original) !== undefined) {
      next++;
      const content = existingContent(copy as IncaElement)!;
      if (deep) {
        for (const child of existingContent(original)!.__children)
          link(content, [build(child as Copyable)], content.__children.length);
      }
    }
    if (!deep) return copy;
    for (const child of original.__children)
      link(copy as IncaParent, [build(child as Copyable)], copy.__children.length);
    return copy;
  };
  const copy = build(node as Copyable);
  keep(copy);
  return copy as T;
}
