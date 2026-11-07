// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { native } from "../native.mts";
import type { NodeId } from "../types.mts";
import type { IncaElement, IncaNode } from "./types.mts";

/** Every handle that `withNodeId` gave a node id. */
const nodes = new WeakSet<object>();

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
  nodes.add(handle);
  return Object.defineProperty(handle, "__node_id", { value: id }) as T & {
    readonly __node_id: NodeId;
  };
}

/**
 * Whether `value` is a node handle.
 * @param value - the value to check
 * @returns `true` for an element, a comment stand-in, a text handle or a fragment
 */
export function isNode(value: unknown): value is IncaNode {
  return typeof value === "object" && value !== null && nodes.has(value);
}

/**
 * Weak references to the node handles, keyed by native node id. A handle
 * that nothing references can be collected, and its record stays until
 * `collected` drops it. A record leaves at once when the host frees the node.
 */
const records = new Map<NodeId, WeakRef<IncaNode>>();

/**
 * Drops the record of a collected handle. A newer handle registered under the
 * same id keeps its record.
 */
const collected = new FinalizationRegistry<NodeId>((id) => {
  if (!records.get(id)?.deref()) records.delete(id);
});

/**
 * Records `node` under its id.
 * @param node - the handle to record
 */
export function registerNode(node: IncaNode): void {
  records.set(node.__node_id, new WeakRef(node));
  collected.register(node, node.__node_id);
}

/**
 * Drops the record of a native node.
 * @param id - the native node's id
 */
export function unregisterNode(id: NodeId): void {
  records.delete(id);
}

/**
 * Counts the recorded nodes, including those whose handle is already collected
 * and waits for the registry to drop it.
 * @returns the number of records
 */
export function recordedCount(): number {
  return records.size;
}

/**
 * Looks up the live handle of a native node of any kind.
 * @param id - the native node's id
 * @returns the handle, or `undefined` when none is live
 */
export function nodeOf(id: NodeId): IncaNode | undefined {
  return records.get(id)?.deref();
}

/**
 * Looks up the live handle of a native element.
 * @param id - the native node's id
 * @returns the handle, or `undefined` when none is live or the node is not an element
 */
export function elementOf(id: NodeId): IncaElement | undefined {
  const node = nodeOf(id);
  return node?.kind === "element" ? node : undefined;
}

const removed = new WeakSet<IncaNode>();

/**
 * Whether `node` left the tree for good, removed or freed. A removed node stays in the host and
 * its members read real values. Its listeners are gone.
 * @param node - the handle to check
 * @returns `true` after {@link markRemoved} or {@link markDestroyed} reached `node`
 */
export function isRemoved(node: IncaNode): boolean {
  return removed.has(node) || destroyed.has(node);
}

/**
 * Records that `node` and every descendant of it were removed from the tree.
 * @param node - the root of the removed subtree
 */
export function markRemoved(node: IncaNode): void {
  removed.add(node);
  if (node.kind === "text") return;
  for (const child of node.__children) markRemoved(child);
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
 * their handles from the registry.
 * @param node - the root of the freed subtree
 */
export function markDestroyed(node: IncaNode): void {
  destroyed.add(node);
  unregisterNode(node.__node_id);
  if (node.kind === "text") return;
  for (const child of node.__children) markDestroyed(child);
}

/**
 * Marks the handle of each freed node destroyed. An id with no live handle is
 * skipped.
 * @param nodeIds - ids of nodes the host freed, subtrees included
 * @internal
 */
export function markFreed(nodeIds: readonly NodeId[]): void {
  for (const id of nodeIds) {
    const node = nodeOf(id);
    if (node) markDestroyed(node);
  }
}

/**
 * Releases the hold of a node whose handle was collected. A node destroyed
 * already is unknown to the host and the call does nothing.
 */
const released = new FinalizationRegistry<NodeId>((id) => native().releaseNode(id));

const held = new WeakSet<IncaNode>();

/**
 * Whether {@link releaseWhenCollected} already covers `handle`.
 * @param handle - the handle to check
 * @returns `true` after `releaseWhenCollected(handle)`
 * @internal
 */
export function isHeld(handle: IncaNode): boolean {
  return held.has(handle);
}

/**
 * Releases the hold of `handle`'s node once the handle is collected.
 * @param handle - the handle of a node held with `holdNode`
 * @internal
 */
export function releaseWhenCollected(handle: IncaNode): void {
  held.add(handle);
  released.register(handle, handle.__node_id);
}
