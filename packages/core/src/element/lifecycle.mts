// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { native } from "../native.mts";
import type { NodeId } from "../rendererCore.mts";
import type { IncaElement, IncaNode } from "./types.mts";

/**
 * Gives `handle` a read-only, non-enumerable `__node_id`.
 * @param handle - the new handle
 * @param id - the native node's id
 * @returns `handle`
 */
export function withNodeId<T extends object>(
  handle: T,
  id: NodeId,
): T & { readonly __node_id: NodeId } {
  return Object.defineProperty(handle, "__node_id", { value: id }) as T & {
    readonly __node_id: NodeId;
  };
}

/**
 * Weak references to the element handles, keyed by native node id. A handle
 * that nothing references can be collected, and its record stays until
 * `collected` drops it. A record leaves at once when the node is removed or
 * the text of its parent is replaced.
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
  elements.set(element.__node_id, new WeakRef(element));
  collected.register(element, element.__node_id);
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
  unregisterElement(node.__node_id);
  for (const child of node.children) markDestroyed(child);
}

/**
 * Marks the handle of each freed node destroyed. An id with no live handle is
 * skipped.
 * @param nodeIds - ids of nodes the host freed, subtrees included
 * @internal
 */
export function markFreed(nodeIds: readonly NodeId[]): void {
  for (const id of nodeIds) {
    const element = elementOf(id);
    if (element) markDestroyed(element);
  }
}

/**
 * Releases the hold of a node whose handle was collected. A node destroyed
 * already is unknown to the host and the call does nothing.
 */
const released = new FinalizationRegistry<NodeId>((id) => native().releaseNode(id));

/**
 * Releases the hold of `handle`'s node once the handle is collected.
 * @param handle - the handle of a node held with `holdNode`
 * @internal
 */
export function releaseWhenCollected(handle: IncaNode): void {
  released.register(handle, handle.__node_id);
}
