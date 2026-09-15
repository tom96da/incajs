// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { releaseCallbacks } from "./events.mts";
import { native } from "./native.mts";
import type { AttributeValue, NodeId, StyleProps, TagName } from "./types.mts";

/**
 * Returns the host-allocated root container's id — the node an app attaches
 * itself under. The host allocates it with the tree, so this resolves from
 * the first call and names the same node for the engine's lifetime.
 * @returns the root node's id
 */
export function rootNodeId(): NodeId {
  return native().rootNodeId();
}

/**
 * Allocates a new node of kind `tag` in the native tree. The node starts out
 * detached — attach it with {@link appendChild}/{@link insertBefore}.
 * @param tag - the node's element kind
 * @returns the new node's id
 */
export function createNode(tag: TagName): NodeId {
  return native().createNode(tag);
}

/**
 * Attaches `childId` as the last child of `parentId`. A thin wrapper over
 * {@link insertBefore} with no anchor.
 * @param parentId - the container to attach to
 * @param childId - the node being attached
 */
export function appendChild(parentId: NodeId, childId: NodeId): void {
  native().appendChild(parentId, childId);
}

/**
 * Attaches `childId` as a child of `parentId`, positioned before `anchorId`
 * (or at the end, if `anchorId` is `null`). If `anchorId` names a real node
 * that just isn't currently a child of `parentId`, this falls back to
 * appending at the end instead of throwing — only a wholly unknown
 * `anchorId` raises.
 * @param parentId - the container to attach to
 * @param childId - the node being attached
 * @param anchorId - the sibling to insert before, or `null` to append at the end
 */
export function insertBefore(parentId: NodeId, childId: NodeId, anchorId: NodeId | null): void {
  native().insertBefore(parentId, childId, anchorId);
}

/**
 * Detaches `childId` from `parentId`. The detached node stays alive (and
 * keeps its own children) — it isn't freed, so it can be re-attached
 * elsewhere. Use {@link destroyNode} for a node that isn't coming back.
 * @param parentId - the current container
 * @param childId - the node being detached
 */
export function removeChild(parentId: NodeId, childId: NodeId): void {
  native().removeChild(parentId, childId);
}

/**
 * Frees `nodeId` and its whole subtree, detaching it first, and drops every
 * event listener registered anywhere in it. Destroying an already-destroyed
 * node does nothing. The host's own root cannot be destroyed and raises.
 *
 * Nothing else frees a node: a subtree dropped with {@link removeChild}
 * alone stays allocated for the life of the engine.
 * @param nodeId - the node that isn't coming back
 */
export function destroyNode(nodeId: NodeId): void {
  releaseCallbacks(native().destroyNode(nodeId));
}

/**
 * Sets a non-style attribute/prop on `nodeId`. See {@link setStyle} for
 * layout/paint properties, which live in a separate map.
 * @param nodeId - the node to update
 * @param key - the attribute name
 * @param value - the attribute value
 */
export function setAttribute(nodeId: NodeId, key: string, value: AttributeValue): void {
  native().setAttribute(nodeId, key, value);
}

/**
 * Sets a style property on `nodeId` — the only way to reach {@link setAttribute}'s
 * counterpart style map. A key from {@link StyleProps} gets compile-time
 * checking on its value's shape; any other string key still forwards as a
 * raw {@link AttributeValue}, for style properties this package's types
 * haven't caught up with yet.
 * @param nodeId - the node to update
 * @param key - the style property name
 * @param value - the style property value
 */
export function setStyle<K extends keyof StyleProps>(
  nodeId: NodeId,
  key: K,
  value: NonNullable<StyleProps[K]>,
): void;
export function setStyle(nodeId: NodeId, key: string, value: AttributeValue): void;
export function setStyle(nodeId: NodeId, key: string, value: AttributeValue): void {
  native().setStyle(nodeId, key, value);
}
