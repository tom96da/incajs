// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { NodeId } from "../rendererCore.mts";
import type { IncaElement, IncaNode } from "./nodeOps.mts";

/**
 * Weak references to the element handles, keyed by native node id. A handle
 * that nothing references can be collected, and its record stays until
 * `collected` drops it. A record leaves at once when Vue removes the node or
 * replaces the text of its parent.
 */
const elements = new Map<NodeId, WeakRef<IncaElement>>();

/**
 * Drops the record of a collected handle. A newer handle registered under the
 * same id keeps its record.
 */
const collected = new FinalizationRegistry<NodeId>((id) => {
  if (!elements.get(id)?.deref()) elements.delete(id);
});

/**
 * Records `element` under its id.
 * @param element - the handle to record
 */
export function registerElement(element: IncaElement): void {
  elements.set(element.id, new WeakRef(element));
  collected.register(element, element.id);
}

/**
 * Drops the record of a native node.
 * @param id - the native node's id
 */
export function unregisterElement(id: NodeId): void {
  elements.delete(id);
}

/**
 * Counts the recorded nodes, including those whose handle is already collected
 * and waits for the registry to drop it.
 * @returns the number of records
 */
export function recordedCount(): number {
  return elements.size;
}

/**
 * Looks up the live handle of a native node.
 * @param id - the native node's id
 * @returns the handle, or `undefined` when none is live
 */
export function elementOf(id: NodeId): IncaElement | undefined {
  return elements.get(id)?.deref();
}

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

/**
 * Marks `handle` so that Vue's `reactive()` and `ref()` keep the object itself.
 * A template ref then holds the same object the application receives from
 * `activeElement()`.
 * @param handle - the node handle to mark
 * @returns `handle`
 */
export function skipReactive<T extends object>(handle: T): T {
  Object.defineProperty(handle, "__v_skip", { value: true });
  return handle;
}
