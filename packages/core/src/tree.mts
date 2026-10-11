// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { releaseCallbacks } from "./events.mts";
import { native } from "./native.mts";
import type { AttributeValue, NodeId, ScrollAlignment, StyleProps, TagName } from "./types.mts";

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
 * Returns the attribute/prop stored on `nodeId` under `key`, or `null` if it
 * isn't set.
 * @param nodeId - the node to read
 * @param key - the attribute name
 * @returns the stored value, or `null`
 */
export function getAttribute(nodeId: NodeId, key: string): AttributeValue | null {
  return native().getAttribute(nodeId, key);
}

/**
 * Returns the box `nodeId` was drawn at in the last frame, in window logical
 * pixels, or `null` if the node has no recorded box.
 * @param nodeId - the node to read
 * @returns `[x, y, width, height, border, scrollX, scrollY]`, or `null`. `border` is one uniform
 *   width on all sides. `scrollX` and `scrollY` are the positive scroll offset the parent applied
 *   to the box.
 */
export function layoutOf(
  nodeId: NodeId,
): readonly [number, number, number, number, number, number, number] | null {
  return native().layoutOf(nodeId);
}

/**
 * Returns the scroll offset of `nodeId` in window logical pixels. A write made
 * since the last draw is included, clamped to the maximum of the last drawn
 * frame.
 * @param nodeId - the node to read
 * @returns `[left, top, maxLeft, maxTop]`, or `null` for a node whose `overflow` is `visible` on both axes, which cannot scroll. The maxima are the largest offsets the node can take, as of the last drawn frame.
 * @throws {TypeError} when `nodeId` names no node
 */
export function scrollOf(nodeId: NodeId): readonly [number, number, number, number] | null {
  return native().scrollOf(nodeId);
}

/**
 * Sets the scroll offset of `nodeId` on one or both axes. A `null` axis keeps
 * its current offset. The host clamps each value to the scrollable range on
 * the next frame. A node that cannot scroll ignores the call.
 * @param nodeId - the node to scroll
 * @param left - the horizontal offset, or `null` to keep it
 * @param top - the vertical offset, or `null` to keep it
 * @throws {TypeError} when `nodeId` names no node
 */
export function scrollNode(nodeId: NodeId, left: number | null, top: number | null): void {
  native().scrollNode(nodeId, left, top);
}

/**
 * Scrolls each scroll container around `nodeId` so the node shows, the way
 * `element.scrollIntoView()` does.
 * @param nodeId - the node to bring into view
 * @param block - the vertical alignment: `"start"`, `"center"`, `"end"` or `"nearest"`
 * @param inline - the horizontal alignment: `"start"`, `"center"`, `"end"` or `"nearest"`
 * @throws {TypeError} when `nodeId` names no node
 * @throws {TypeError} when `block` or `inline` is not an alignment name
 */
export function scrollIntoViewNode(
  nodeId: NodeId,
  block: ScrollAlignment,
  inline: ScrollAlignment,
): void {
  native().scrollIntoViewNode(nodeId, block, inline);
}

/**
 * Removes a non-style attribute/prop from `nodeId`. Removing an attribute
 * that isn't set does nothing.
 * @param nodeId - the node to update
 * @param key - the attribute name
 */
export function removeAttribute(nodeId: NodeId, key: string): void {
  native().removeAttribute(nodeId, key);
}

/**
 * Sets a style property on `nodeId`. A snake_case key from {@link StyleProps}
 * gets compile-time checking on its value's shape. Any other string key, such
 * as a camelCase or kebab-case spelling or a property the types do not list,
 * forwards as a raw {@link AttributeValue}.
 *
 * A key takes any of three spellings: kebab-case (`flex-direction`),
 * camelCase (`flexDirection`) or snake_case (`flex_direction`). All three
 * name the same property, and the value set most recently applies.
 * @param nodeId - the node to update
 * @param key - the style property name in any of the three spellings
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

/**
 * Removes a style property from `nodeId`, so it renders as if never set.
 * Removing a property that isn't set does nothing. The key takes any of the
 * three spellings {@link setStyle} accepts, and removes the property whichever
 * spelling set it.
 * @param nodeId - the node to update
 * @param key - the style property name in any of the three spellings
 */
export function removeStyle(nodeId: NodeId, key: string): void {
  native().removeStyle(nodeId, key);
}
