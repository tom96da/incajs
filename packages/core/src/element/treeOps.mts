// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The tree operations a renderer calls. The host holds the tree, and these functions keep the node
// table in step with it.

import { releaseCallbacks } from "../callbacks.mts";
import { insertBefore, linkedNode, removeNode } from "../ffi/index.mts";
import { handleOf } from "./handle.mts";
import { entered, isDestroyed, leftHeld, markRemoved } from "./lifecycle.mts";
import { notFound } from "./nodeMutation.mts";
import type { HandleNode } from "./handleNode.mts";
import type { Node } from "./nodeTypes.mts";
import type { Element } from "./types.mts";

/**
 * Attaches `child` to `parent`, before `anchor` or at the end when `anchor` is
 * omitted or `null`. If `anchor` is `child` itself and `child` is already in
 * `parent`, nothing changes. A `child` attached elsewhere is detached first,
 * so a move keeps the node alive.
 * @param child - the node being attached
 * @param parent - the new container
 * @param anchor - the sibling to insert before, or omitted/`null` to append at the end
 * @throws {DOMException} `NotFoundError` when `anchor` was freed or is no child of `parent`
 */
export function insert(child: HandleNode, parent: Element, anchor?: Node | null): void {
  if (anchor && isDestroyed(anchor)) throw notFound();
  insertBefore(parent.__node_id, child.__node_id, anchor?.__node_id ?? null);
  if (child.kind !== "fragment") entered(child);
}

/**
 * Removes `child` for good: the native node leaves its parent and keeps its
 * subtree, and every event listener registered in the subtree is released. The
 * handles stay readable: the node is held until its handle is collected. A freed
 * `child` changes nothing.
 *
 * A caller that removes a subtree calls this on its root alone.
 * @param child - the node being removed
 */
export function remove(child: Node): void {
  if (isDestroyed(child)) return;
  const [callbackIds, nodeIds] = removeNode(child.__node_id);
  releaseCallbacks(callbackIds);
  markRemoved(nodeIds);
  leftHeld(child, nodeIds);
}

/**
 * Looks up the current parent.
 * @param node - the node to query
 * @returns the parent element, or `null` when `node` has no parent, has a fragment or the document
 *   as its parent, or was freed
 */
export function parentNode(node: Node): Element | null {
  if (isDestroyed(node)) return null;
  const id = linkedNode(node.__node_id, "parent", true);
  return id === null ? null : ((handleOf(id) as Element | undefined) ?? null);
}

/**
 * Looks up the next sibling.
 * @param node - the node to query
 * @returns the sibling immediately after `node` in its parent's children, or `null` when there is
 *   none, `node` has no parent or it was freed
 */
export function nextSibling(node: Node): Node | null {
  if (isDestroyed(node)) return null;
  const id = linkedNode(node.__node_id, "next");
  return id === null ? null : (handleOf(id) ?? null);
}
