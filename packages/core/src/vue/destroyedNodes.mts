// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { unregisterElement } from "./elements.mts";
import type { IncaNode } from "./nodeOps.mts";

const destroyed = new WeakSet<IncaNode>();

/**
 * Whether the host freed `node` with its subtree.
 * @param node - the handle to check
 * @returns `true` after {@link markDestroyed} reached `node`
 */
export function isDestroyed(node: IncaNode): boolean {
  return destroyed.has(node);
}

/**
 * Records that the host freed `node` and every descendant of it, and drops
 * their element handles from the registry.
 * @param node - the root of the freed subtree
 */
export function markDestroyed(node: IncaNode): void {
  destroyed.add(node);
  if (node.kind === "text") return;
  unregisterElement(node.id);
  for (const child of node.children) markDestroyed(child);
}
