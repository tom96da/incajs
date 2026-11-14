// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { HTML_NAMESPACE, native } from "./native.mts";
import type { CallbackId, NodeId, TagName } from "../types.mts";
import type {
  ElementEntry,
  ElementName,
  NodeEntry,
  NodeLink,
  Parsed,
  ParsedEntry,
} from "./native.mts";

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
 * Returns the id of the document node, the parent of the root container. The host allocates it
 * with the tree and it names the same node for the engine's lifetime.
 * @returns the document node's id
 */
export function documentNodeId(): NodeId {
  return native().documentNodeId();
}

/**
 * Allocates a new element in the native tree, as {@link createNode} does for a tag, after the
 * host checks that `localName` is a valid element name and lowers its ASCII upper case.
 * @param localName - the element name
 * @returns the new node's id and its tag
 * @throws {DOMException} `InvalidCharacterError` when `localName` is not a valid element name
 */
export function createElement(localName: string): [NodeId, TagName] {
  return native().createElement(localName);
}

/**
 * Allocates a new element in the native tree with a namespace, as {@link createElement} does for
 * the HTML namespace. The host validates and splits `qualifiedName`; JS converts the arguments.
 * @param namespace - the namespace, or `null` for none
 * @param qualifiedName - the element name, with an optional prefix
 * @returns the new node's id, local name, namespace, prefix and tag name
 * @throws {DOMException} `InvalidCharacterError` when a part of the name is not valid, and
 *   `NamespaceError` when the prefix and the namespace do not fit
 */
export function createElementNS(namespace: string | null, qualifiedName: string): ElementEntry {
  return native().createElementNS(namespace, qualifiedName);
}

/**
 * Reads the name a creating native returned.
 * @param entry - an entry of `createElement`, `createElementNS` or `cloneNode`
 * @returns the tag string for a two-part entry, and the last four parts for a five-part entry
 */
export function nameOf(entry: NodeEntry | ElementEntry): ElementName {
  return entry.length === 2 ? entry[1] : [entry[1], entry[2], entry[3], entry[4]];
}

/**
 * Reads the name of an element row of a parse result. A parsed element has no prefix, and a name
 * outside the HTML namespace keeps its case.
 * @param row - an element row
 * @returns the tag string for an HTML row, and the name tuple for any other namespace
 */
export function nameOfRow(row: ParsedEntry): ElementName {
  return row[2] === HTML_NAMESPACE ? row[1] : [row[1], row[2], null, row[1]];
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
 *   `nodeId` is `parentId` or an ancestor of it, `nodeId` is the document, or `parentId` is the
 *   document and the insertion gives it text or a second element;
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
 * @returns one entry per copy in pre-order, the copy of `nodeId` first: the id and the four name
 *   parts of an element, the id and the marker of any other node
 * @throws {TypeError} when `nodeId` names no node
 */
export function cloneNode(nodeId: NodeId, deep: boolean): (NodeEntry | ElementEntry)[] {
  return native().cloneNode(nodeId, deep);
}

/**
 * Reads the markup of the children of a node. A template reads its content.
 * @param nodeId - the node to read
 * @returns the markup
 * @throws {TypeError} when `nodeId` names no node
 */
export function innerHtml(nodeId: NodeId): string {
  return native().innerHtml(nodeId);
}

/**
 * Reads the markup of a node and its children.
 * @param nodeId - the node to read
 * @returns the markup
 * @throws {TypeError} when `nodeId` names no node
 */
export function outerHtml(nodeId: NodeId): string {
  return native().outerHtml(nodeId);
}

/**
 * Parses `html` in the context of a node and replaces its children, or the children of its content
 * for a template. The old children keep their listeners and are freed at the start of the next
 * frame unless held.
 * @param nodeId - the node whose children are replaced
 * @param html - the markup
 * @returns the parsed nodes, put at position 0
 * @throws {TypeError} when `nodeId` names no node
 */
export function setInnerHtml(nodeId: NodeId, html: string): Parsed {
  return native().setInnerHtml(nodeId, html);
}

/**
 * Parses `html` in the context of the parent of a node and replaces the node with the result.
 * @param nodeId - the node to replace
 * @param html - the markup
 * @returns the parsed nodes and their position, or `null` and no change for a node without a parent
 * @throws {DOMException} `NoModificationAllowedError` for a node whose parent is the document
 * @throws {TypeError} when `nodeId` names no node
 */
export function setOuterHtml(nodeId: NodeId, html: string): Parsed | null {
  return native().setOuterHtml(nodeId, html);
}

/**
 * Parses `html` and inserts the nodes next to or inside a node.
 * @param nodeId - the node the position is relative to
 * @param position - `beforebegin`, `afterbegin`, `beforeend` or `afterend`, in any ASCII case
 * @param html - the markup
 * @returns the parsed nodes and their position among the children of their new parent
 * @throws {DOMException} `SyntaxError` for another position; `NoModificationAllowedError` for
 *   `beforebegin` or `afterend` on a node without a parent or a node whose parent is the document
 * @throws {TypeError} when `nodeId` names no node
 */
export function insertAdjacentHtml(nodeId: NodeId, position: string, html: string): Parsed {
  return native().insertAdjacentHtml(nodeId, position, html);
}

/**
 * Finds the top of the tree `nodeId` belongs to.
 * @param nodeId - the node to start from
 * @returns the id of the topmost ancestor, which is the document for a connected node, and
 *   `nodeId` itself without a parent
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
 * @returns the callback ids of the dropped listeners
 */
export function releaseListeners(nodeId: NodeId): CallbackId[] {
  return native().releaseListeners(nodeId);
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
 * Reads the content of a template.
 * @param nodeId - the template
 * @returns the id of the fragment that holds the content, or `null` for a node that is no template
 * @throws {TypeError} when `nodeId` names no node
 */
export function templateContent(nodeId: NodeId): NodeId | null {
  return native().templateContent(nodeId);
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
 * Whether the parent chain of a node reaches the document.
 * @param nodeId - the node to test
 * @returns `true` for the document and the nodes below it
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
