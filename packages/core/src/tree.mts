// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { releaseCallbacks } from "./callbacks.mts";
import { native } from "./native.mts";
import type { NodeLink } from "./native.mts";
import type { AttributeValue, NodeId, ScrollAlignment, StyleProps, TagName } from "./types.mts";

/**
 * Returns the host-allocated root container's id, the node an app attaches
 * itself under. The host allocates it with the tree, so this resolves from
 * the first call and names the same node for the engine's lifetime.
 * @returns the root node's id
 */
export function rootNodeId(): NodeId {
  return native().rootNodeId();
}

/**
 * Allocates a new node of kind `tag` in the native tree. The node starts out
 * detached. Attach it with {@link appendChild}/{@link insertBefore}. The host
 * frees the node at the start of the next frame unless it is inserted or held
 * with {@link holdNode} by then.
 * @param tag - the node's element kind
 * @returns the new node's id
 */
export function createNode(tag: TagName): NodeId {
  return native().createNode(tag);
}

/**
 * Attaches `nodeId` as the last child of `parentId`. A thin wrapper over
 * {@link insertBefore} with no anchor.
 * @param parentId - the container to attach to
 * @param nodeId - the node being attached, or a fragment whose children are attached
 * @returns the position of the first attached node among the children of `parentId`
 * @throws {DOMException} `HierarchyRequestError` as {@link insertBefore} does
 */
export function appendChild(parentId: NodeId, nodeId: NodeId): number {
  return native().appendChild(parentId, nodeId);
}

/**
 * Attaches `nodeId` as a child of `parentId`, positioned before `refId`
 * (or at the end, if `refId` is `null`). A fragment attaches its children in order.
 * @param parentId - the container to attach to
 * @param nodeId - the node being attached, or a fragment whose children are attached
 * @param refId - the sibling to insert before, or `null` to append at the end
 * @returns the position of the first attached node among the children of `parentId`
 * @throws {DOMException} `HierarchyRequestError` when `parentId` is a text node or a comment,
 *   `nodeId` is `parentId` or an ancestor of it, or `nodeId` is the root container;
 *   `NotFoundError` when `refId` is not a child of `parentId`
 * @throws {TypeError} when an id names no node
 */
export function insertBefore(parentId: NodeId, nodeId: NodeId, refId: NodeId | null): number {
  return native().insertBefore(parentId, nodeId, refId);
}

/**
 * Detaches `childId` from `parentId`. The detached node keeps its own children and its listeners,
 * and is freed at the start of the next frame unless it is inserted or held
 * with {@link holdNode} by then. An unknown `childId` does nothing.
 * @param parentId - the current container
 * @param childId - the node being detached
 * @throws {DOMException} `NotFoundError` when `childId` is a node that is not a child of `parentId`
 * @throws {TypeError} when `parentId` names no node
 */
export function removeChild(parentId: NodeId, childId: NodeId): void {
  native().removeChild(parentId, childId);
}

/**
 * Puts `nodeId` where `childId` is a child of `parentId`. `childId` leaves the tree with its
 * subtree, keeps its listeners and is freed at the start of the next frame unless it is held with
 * {@link holdNode}. A fragment puts its children there in order.
 * @param parentId - the container that holds `childId`
 * @param nodeId - the node being attached, or a fragment whose children are attached
 * @param childId - the child being replaced
 * @returns the position of the first attached node among the children of `parentId`
 * @throws {DOMException} `HierarchyRequestError` and `NotFoundError` as {@link insertBefore} does,
 *   with `childId` as the node that has to be a child
 * @throws {TypeError} when an id names no node
 */
export function replaceChild(parentId: NodeId, nodeId: NodeId, childId: NodeId): number {
  return native().replaceChild(parentId, nodeId, childId);
}

/**
 * Copies `nodeId` in the host tree. The copy has no parent and is freed at the start of the next
 * frame unless it is held with {@link holdNode}. It keeps the tag, attributes and style of the
 * original and the typed text of an `input`, and no listeners.
 * @param nodeId - the node to copy
 * @param deep - whether the descendants are copied too
 * @returns the id and tag of each copy in pre-order, the copy of `nodeId` first
 * @throws {TypeError} when `nodeId` names no node
 */
export function cloneNode(nodeId: NodeId, deep: boolean): [NodeId, TagName][] {
  return native().cloneNode(nodeId, deep);
}

/**
 * Finds the top of the tree `nodeId` belongs to.
 * @param nodeId - the node to start from
 * @returns the id of the topmost ancestor, `nodeId` itself without a parent
 * @throws {TypeError} when `nodeId` names no node
 */
export function rootOf(nodeId: NodeId): NodeId {
  return native().rootOf(nodeId);
}

/**
 * Compares the tree positions of two nodes.
 * @param nodeId - the node the mask describes the other from
 * @param otherId - the node to locate
 * @returns the bit mask of `compareDocumentPosition`
 * @throws {TypeError} when an id names no node
 */
export function compareNodePosition(nodeId: NodeId, otherId: NodeId): number {
  return native().compareNodePosition(nodeId, otherId);
}

/**
 * Compares two nodes and their subtrees by tag, attributes, style and children.
 * @param nodeId - the first node
 * @param otherId - the second node
 * @returns whether the two are equal
 * @throws {TypeError} when an id names no node
 */
export function isEqualNode(nodeId: NodeId, otherId: NodeId): boolean {
  return native().isEqualNode(nodeId, otherId);
}

/**
 * Drops every event listener registered on `nodeId` and on the nodes below it. The nodes stay in
 * the tree. An unknown id does nothing.
 * @param nodeId - the root of the subtree
 */
export function releaseListeners(nodeId: NodeId): void {
  releaseCallbacks(native().releaseListeners(nodeId));
}

/**
 * Keeps `nodeId` and its subtree alive while it has no parent. The host frees
 * a parentless node that is neither held nor inserted by the start of the next
 * frame. Holding a held or attached node changes nothing visible.
 * @param nodeId - the node to keep
 * @throws {TypeError} when `nodeId` names no node
 */
export function holdNode(nodeId: NodeId): void {
  native().holdNode(nodeId);
}

/**
 * Ends the hold of {@link holdNode}. A node with no parent is freed at the
 * start of the next frame. An unknown id does nothing.
 * @param nodeId - the node to let go
 */
export function releaseNode(nodeId: NodeId): void {
  native().releaseNode(nodeId);
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
 * Returns the attribute/prop stored on `nodeId` under `key`, or `null` when unset.
 * @param nodeId - the node to read
 * @param key - the attribute name
 * @returns the stored value, or `null`
 * @throws {TypeError} when `nodeId` names no node
 */
export function getAttribute(nodeId: NodeId, key: string): AttributeValue | null {
  return native().getAttribute(nodeId, key);
}

/**
 * Returns the names of the attributes stored on `nodeId` in the order they were set, in the lower
 * case the host keeps them in.
 * @param nodeId - the node to read
 * @returns the attribute names
 * @throws {TypeError} when `nodeId` names no node
 */
export function attributeNames(nodeId: NodeId): string[] {
  return native().attributeNames(nodeId);
}

/**
 * Reads one link of a node in the host tree.
 * @param nodeId - the node to start from
 * @param link - which node to reach
 * @returns the id of the parent, first child, last child, previous sibling or next sibling, or
 *   `null` when there is none
 * @throws {TypeError} when `nodeId` names no node
 */
export function linkedNode(nodeId: NodeId, link: NodeLink): NodeId | null {
  return native().linkedNode(nodeId, link);
}

/**
 * Lists the children of a node in the host tree.
 * @param nodeId - the parent
 * @returns the ids of the children in order
 * @throws {TypeError} when `nodeId` names no node
 */
export function childNodeIds(nodeId: NodeId): NodeId[] {
  return native().childNodeIds(nodeId);
}

/**
 * Whether the host tree connects a node to the mount container.
 * @param nodeId - the node to test
 * @returns `true` for the mount container and the nodes below it
 * @throws {TypeError} when `nodeId` names no node
 */
export function isConnected(nodeId: NodeId): boolean {
  return native().isConnected(nodeId);
}

/**
 * Reads the text of a node in the host tree: the data of a text or comment node, and the data of
 * the text descendants in tree order for an element.
 * @param nodeId - the node to read
 * @returns the text
 * @throws {TypeError} when `nodeId` names no node
 */
export function textContent(nodeId: NodeId): string {
  return native().textContent(nodeId);
}

/**
 * Reads the data of a text node and of the text siblings that touch it.
 * @param nodeId - the text node
 * @returns the joined data in order
 * @throws {TypeError} when `nodeId` names no node
 */
export function wholeText(nodeId: NodeId): string {
  return native().wholeText(nodeId);
}

/**
 * Reads a range of the data of a node. Offset and count convert to unsigned 32-bit integers and
 * count UTF-16 code units.
 * @param nodeId - the text or comment node
 * @param offset - where the range starts
 * @param count - the length of the range, cut at the end of the data
 * @returns the text of the range
 * @throws {DOMException} named `IndexSizeError` when `offset` is beyond the data
 * @throws {TypeError} when `nodeId` names no node
 */
export function substringData(nodeId: NodeId, offset: number, count: number): string {
  return native().substringData(nodeId, offset, count);
}

/**
 * Replaces a range of the data of a node. Offset and count convert like in {@link substringData}.
 * @param nodeId - the text or comment node
 * @param offset - where the range starts
 * @param count - the length of the range, cut at the end of the data
 * @param data - the text that takes the place of the range
 * @throws {DOMException} named `IndexSizeError` when `offset` is beyond the data
 * @throws {TypeError} when `nodeId` names no node
 */
export function replaceData(nodeId: NodeId, offset: number, count: number, data: string): void {
  native().replaceData(nodeId, offset, count, data);
}

/**
 * Cuts the data of a text node. The new node holds the tail and follows `nodeId` when `nodeId` has
 * a parent. It has no parent otherwise, and the host frees it at the start of the next frame
 * unless it is inserted or held with {@link holdNode} by then.
 * @param nodeId - the text node
 * @param offset - where to cut, converted like in {@link substringData}
 * @returns the id of the new node
 * @throws {DOMException} named `IndexSizeError` when `offset` is beyond the data
 * @throws {TypeError} when `nodeId` names no node
 */
export function splitText(nodeId: NodeId, offset: number): NodeId {
  return native().splitText(nodeId, offset);
}

/**
 * Whether the element `nodeId` matches the selector list `selector`.
 * @param nodeId - the node to test
 * @param selector - the selector list
 * @returns `true` for a match
 * @throws {DOMException} named `SyntaxError` for an invalid selector
 * @throws {TypeError} when `nodeId` names no node
 */
export function matchesNode(nodeId: NodeId, selector: string): boolean {
  return native().matches(nodeId, selector);
}

/**
 * Finds the nearest element at or above `nodeId` that matches `selector`.
 * @param nodeId - the node to start at
 * @param selector - the selector list
 * @returns the id of the match, or `null`
 * @throws {DOMException} named `SyntaxError` for an invalid selector
 * @throws {TypeError} when `nodeId` names no node
 */
export function closestNode(nodeId: NodeId, selector: string): NodeId | null {
  return native().closest(nodeId, selector);
}

/**
 * Finds the first descendant of `nodeId` that matches `selector`, in document order.
 * @param nodeId - the node to search below
 * @param selector - the selector list
 * @returns the id of the match, or `null`
 * @throws {DOMException} named `SyntaxError` for an invalid selector
 * @throws {TypeError} when `nodeId` names no node
 */
export function queryNode(nodeId: NodeId, selector: string): NodeId | null {
  return native().querySelector(nodeId, selector);
}

/**
 * Finds every descendant of `nodeId` that matches `selector`, in document order.
 * @param nodeId - the node to search below
 * @param selector - the selector list
 * @returns the ids of the matches
 * @throws {DOMException} named `SyntaxError` for an invalid selector
 * @throws {TypeError} when `nodeId` names no node
 */
export function queryNodes(nodeId: NodeId, selector: string): NodeId[] {
  return native().querySelectorAll(nodeId, selector);
}

/**
 * Checks that `selector` is a valid selector list, without a node.
 * @param selector - the selector list
 * @throws {DOMException} named `SyntaxError` for an invalid selector
 */
export function checkSelector(selector: string): void {
  native().checkSelector(selector);
}

/**
 * Returns the box `nodeId` was drawn at in the last frame, in window logical
 * pixels, or `null` if the node has no recorded box.
 * @param nodeId - the node to read
 * @returns `[x, y, width, height, border, scrollX, scrollY]`, or `null`. `border` is one uniform
 *   width on all sides. `scrollX` and `scrollY` are the positive scroll offset the parent applied
 *   to the box.
 * @throws {TypeError} when `nodeId` names no node
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
 * @returns `[left, top, maxLeft, maxTop]`, or `null` for a node whose `overflow` is
 *   `visible` or `clip` on both axes. The maxima are the largest offsets the node can take, as
 *   of the last drawn frame.
 * @throws {TypeError} when `nodeId` names no node
 */
export function scrollOf(nodeId: NodeId): readonly [number, number, number, number] | null {
  return native().scrollOf(nodeId);
}

/**
 * Sets the scroll offset of `nodeId` on one or both axes. A `null` axis keeps
 * its current offset. The host clamps each value to the scrollable range on
 * the next frame. A node whose `overflow` is `visible` or `clip` on both axes ignores the call.
 * @param nodeId - the node to scroll
 * @param left - the horizontal offset, or `null` to keep it
 * @param top - the vertical offset, or `null` to keep it
 * @throws {TypeError} when `nodeId` names no node
 */
export function scrollNode(nodeId: NodeId, left: number | null, top: number | null): void {
  native().scrollNode(nodeId, left, top);
}

/**
 * Scrolls each scroll container around `nodeId` so the node shows.
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
 * Sets a style property on `nodeId`. A key from {@link StyleProps} gets
 * compile-time checking on its value's shape. Any other string key, such as a
 * camelCase or kebab-case spelling or a property the types do not list,
 * forwards as a raw {@link AttributeValue}.
 *
 * A key takes kebab-case (`flex-direction`) or camelCase (`flexDirection`).
 * Both spellings share one slot per property, and the value set most recently
 * applies.
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

/**
 * Removes a style property from `nodeId`, so it renders as if never set.
 * Removing a property that isn't set does nothing. The key takes any spelling
 * or CSS name {@link setStyle} accepts, and removes the property whichever
 * one set it.
 * @param nodeId - the node to update
 * @param key - the style property name
 */
export function removeStyle(nodeId: NodeId, key: string): void {
  native().removeStyle(nodeId, key);
}

/**
 * Returns the text of the `input` node `nodeId`.
 * @param nodeId - the `input` node to read
 * @returns the text of the input
 * @throws TypeError for a node that is not an input
 */
export function inputValue(nodeId: NodeId): string {
  return native().inputValue(nodeId);
}

/**
 * Replaces the text of the `input` node `nodeId`. Line breaks are dropped, and a changed text
 * puts the caret at its end.
 * @param nodeId - the `input` node to update
 * @param value - the new text
 * @throws TypeError for a node that is not an input
 */
export function setInputValue(nodeId: NodeId, value: string): void {
  native().setInputValue(nodeId, value);
}

/**
 * Returns the id of the focused node.
 * @returns the focused node's id, or `null` when no node has focus
 */
export function focusedNode(): NodeId | null {
  return native().focusedNode();
}
