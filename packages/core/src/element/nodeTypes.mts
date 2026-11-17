// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { Document, EventTarget, NodeId } from "../types.mts";
import type { Element, HTMLCollectionOf, NodeListOf, Text } from "./types.mts";

/** The sibling members of the nodes that have a parent and are no document type. */
export interface NonDocumentTypeChildNode extends Node {
  /** The closest earlier sibling that is an element, or `null`. */
  readonly previousElementSibling: Element | null;
  /** The closest later sibling that is an element, or `null`. */
  readonly nextElementSibling: Element | null;
}

/** A node of the tree: what it is, its data and its place in the tree. */
export interface Node extends EventTarget {
  /**
   * The node id the host knows this node by. Internal to the renderer.
   * @internal
   */
  readonly __node_id: NodeId;
  /**
   * `1` for an element, `3` for a text node, `8` for a comment, `9` for the document and
   * `11` for a fragment.
   */
  readonly nodeType: 1 | 3 | 8 | 9 | 11;
  /**
   * The {@link Element.tagName} of an element, `"#text"` for a text node, `"#comment"` for a
   * comment, `"#document"` for the document and `"#document-fragment"` for a fragment.
   */
  readonly nodeName: string;
  /**
   * The data of a text node or a comment, and `null` for an element, the document or a
   * fragment. Assigning `null` or `undefined` stores `""` and any other value stores its string.
   * Assigning to an element, the document, a fragment or a freed node does nothing.
   */
  nodeValue: string | null;
  /** Whether the node hangs under `document`. */
  readonly isConnected: boolean;
  /** `document`, and `null` for `document` itself. */
  readonly ownerDocument: Document | null;
  /**
   * The parent, or `null` for a detached node. A node inside a fragment reads the fragment. A
   * freed node reads `null`.
   */
  readonly parentNode: Node | null;
  /**
   * The parent element, or `null` for a detached node, a node inside a fragment and the mount
   * container.
   */
  readonly parentElement: Element | null;
  /** The first child, or `null`. */
  readonly firstChild: Node | null;
  /** The last child, or `null`. */
  readonly lastChild: Node | null;
  /** The sibling right before this node, or `null`. */
  readonly previousSibling: Node | null;
  /** The sibling right after this node, or `null`. */
  readonly nextSibling: Node | null;
  /**
   * The children in order, as a live list. Every read returns the same object, which follows later
   * changes. A freed node has an empty list.
   */
  readonly childNodes: NodeListOf<Node>;
  /**
   * The data of a text node or a comment, and the data of the text nodes below an
   * element in tree order, comments left out. Assigning `null` or `undefined` stores `""`
   * and any other value stores its string. Assigning to an element replaces its children with one
   * text node, or with none for `""`. A freed node reads `""` or its last data and ignores the
   * assignment.
   */
  textContent: string | null;
  /** Whether the node has a child. */
  hasChildNodes(): boolean;
  /**
   * Appends `node` as the last child. A fragment gives up its children in order and stays empty.
   * A node that has a parent moves. Throws `HierarchyRequestError` when this node is a text node or
   * a comment, or when `node` is this node or one of its ancestors.
   * @returns `node`
   */
  appendChild<T extends Node>(node: T): T;
  /**
   * Inserts `node` before `child`, or last when `child` is `null` or `undefined`. A fragment gives
   * up its children in order and stays empty. Throws `HierarchyRequestError` as
   * {@link Node.appendChild} does, and `NotFoundError` when `child` is not a child of this
   * node.
   * @returns `node`
   */
  insertBefore<T extends Node>(node: T, child: Node | null | undefined): T;
  /**
   * Removes `child`, which keeps its listeners and can be inserted again. Throws `NotFoundError`
   * when `child` is not a child of this node.
   * @returns `child`
   */
  removeChild<T extends Node>(child: T): T;
  /**
   * Puts `node` in the place of `child`, which keeps its listeners and can be inserted again.
   * Throws as {@link Node.insertBefore} does, with `child` as the node that has to be a
   * child of this node.
   * @returns `child`
   */
  replaceChild<T extends Node>(node: Node, child: T): T;
  /**
   * Copies the node: its tag, attributes and style, and with `deep` its descendants. The copy has
   * no parent, no listeners and no scroll offset, and the text of an `input` carries over.
   * @param deep - whether the descendants are copied too
   * @returns the copy
   */
  cloneNode(deep?: boolean): this;
  /**
   * Whether `other` is this node or one of its descendants in the tree as rendered. `null` and
   * `undefined` give `false`, and any other value that is not a node throws a `TypeError`. After
   * the node is removed, it still answers for the subtree it keeps. A freed node contains only
   * itself.
   */
  contains(other: Node | null): boolean;
  /**
   * The topmost node above this one: `document` for a connected node, else the top of its
   * subtree, the node itself without a parent.
   */
  getRootNode(): Node;
  /**
   * The position of `other` relative to this node as a bit mask: `1` disconnected, `2` preceding,
   * `4` following, `8` contains, `16` contained by and `32` implementation specific. Nodes of
   * different trees give `1 | 32` plus `2` or `4`, with opposite directions for the two orders of
   * the pair. A freed node is a tree of its own: it gives `1 | 32` plus `2` or `4` by the order of
   * the two ids, and `0` for itself.
   * @throws {TypeError} when `other` is not a node
   */
  compareDocumentPosition(other: Node): number;
  /** Whether `other` is this node. `null` and `undefined` give `false`. */
  isSameNode(other: Node | null): boolean;
  /**
   * Whether `other` has the same tag, attributes, style and children, compared in depth. The typed
   * text of an `input` is not compared. `null` and `undefined` give `false`. A freed node equals
   * only itself.
   */
  isEqualNode(other: Node | null): boolean;
}

/** The members of a text node and a comment that read and edit their data. */
export interface CharacterData extends Node {
  /** The data. Assigning `null` stores `""` and any other value stores its string. */
  data: string;
  /** The number of UTF-16 code units of the data. */
  readonly length: number;
  /**
   * The part of the data from `offset`. Both arguments convert to unsigned 32-bit integers and
   * count UTF-16 code units. `count` stops at the end of the data.
   * @param offset - Where the part starts.
   * @param count - The length of the part.
   * @throws {DOMException} named `IndexSizeError` when `offset` is beyond the data.
   */
  substringData(offset: number, count: number): string;
  /**
   * Adds `data` at the end of the data.
   * @param data - The text to add.
   */
  appendData(data: string): void;
  /**
   * Adds `data` at `offset`.
   * @param offset - Where to add.
   * @param data - The text to add.
   * @throws {DOMException} named `IndexSizeError` when `offset` is beyond the data.
   */
  insertData(offset: number, data: string): void;
  /**
   * Removes `count` units from `offset`.
   * @param offset - Where the removal starts.
   * @param count - The number of units, stopping at the end of the data.
   * @throws {DOMException} named `IndexSizeError` when `offset` is beyond the data.
   */
  deleteData(offset: number, count: number): void;
  /**
   * Replaces `count` units from `offset` with `data`.
   * @param offset - Where the replaced part starts.
   * @param count - The number of units, stopping at the end of the data.
   * @param data - The new text.
   * @throws {DOMException} named `IndexSizeError` when `offset` is beyond the data.
   */
  replaceData(offset: number, count: number, data: string): void;
}

/** The members only a text node has. */
export interface TextMembers {
  /** The data of this node and of the text nodes that touch it, in order. */
  readonly wholeText: string;
  /**
   * Cuts the data at `offset`. This node keeps the head and the new node holds the tail. The new
   * node follows this one when this one has a parent, and has no parent otherwise.
   * @param offset - Where to cut, converted like the offset of `substringData()`.
   * @returns The new node.
   * @throws {DOMException} named `IndexSizeError` when `offset` is beyond the data.
   */
  splitText(offset: number): Text;
}

/** The members of the nodes that hold children that insert and replace nodes. */
export interface ParentNodeMutationMembers {
  /**
   * Inserts `nodes` after the last child. A string becomes a new text node. A fragment gives up
   * its children in order, and a node that has a parent moves. With several nodes, a node given
   * twice takes the place of its last occurrence. When the call throws, no node has moved.
   * @param nodes - the nodes and strings to insert
   * @throws {DOMException} named `HierarchyRequestError` when a node is this node or one of its
   * ancestors, or the insertion gives the document text or a second element
   * @throws {TypeError} for a `Symbol`, a string with a lone surrogate or a freed node
   */
  append(...nodes: (Node | string)[]): void;
  /**
   * Inserts `nodes` before the first child that the call does not move. Takes and throws as
   * {@link ParentNodeMutationMembers.append} does.
   * @param nodes - the nodes and strings to insert
   */
  prepend(...nodes: (Node | string)[]): void;
  /**
   * Makes `nodes` the only children. Every other child leaves, keeps its listeners and can be
   * inserted again. Takes and throws as {@link ParentNodeMutationMembers.append} does. A document
   * that holds an element throws when it gets an element.
   * @param nodes - the nodes and strings to insert
   */
  replaceChildren(...nodes: (Node | string)[]): void;
}

/** The members of the nodes that have a parent and leave it or change places in it. */
export interface ChildNode extends Node {
  /**
   * Inserts `nodes` before this node. A node that has no parent does nothing. Takes and throws as
   * {@link ParentNodeMutationMembers.append} does.
   * @param nodes - the nodes and strings to insert
   */
  before(...nodes: (Node | string)[]): void;
  /**
   * Inserts `nodes` after this node. A node that has no parent does nothing. Takes and throws as
   * {@link ParentNodeMutationMembers.append} does.
   * @param nodes - the nodes and strings to insert
   */
  after(...nodes: (Node | string)[]): void;
  /**
   * Puts `nodes` in the place of this node, which leaves its parent unless it is one of them. A
   * node that has no parent does nothing. Takes and throws as
   * {@link ParentNodeMutationMembers.append} does.
   * @param nodes - the nodes and strings to insert
   */
  replaceWith(...nodes: (Node | string)[]): void;
  /** Takes this node out of its parent. It keeps its listeners and can be inserted again. */
  remove(): void;
}

/** The members of the nodes that hold children: the child elements and the queries. */
export interface ParentNode extends Node, ParentNodeMutationMembers {
  /**
   * The child elements in order, as a live collection. Every read returns the same object, which
   * follows later changes. A removed element lists the element children it kept.
   */
  readonly children: HTMLCollectionOf<Element>;
  /** The first child that is an element, or `null`. */
  readonly firstElementChild: Element | null;
  /** The last child that is an element, or `null`. */
  readonly lastElementChild: Element | null;
  /** The number of children that are elements. */
  readonly childElementCount: number;
  /**
   * The first descendant element that matches the selector list, or `null`. The node itself is
   * never in the result. `:scope` and a leading `&` select an element receiver. A removed element
   * searches the subtree it kept.
   * @throws DOMException named `SyntaxError` for an invalid selector, also after the node is
   * removed
   * @param selectors - A selector list.
   * @returns The first matching descendant in document order, or `null`.
   */
  querySelector(selectors: string): Element | null;
  /**
   * The descendant elements that match the selector list, in document order, as a list that later
   * changes do not reach. Every call returns a new list. A removed element searches the subtree it
   * kept.
   * @throws DOMException named `SyntaxError` for an invalid selector, also after the node is
   * removed
   * @param selectors - A selector list.
   * @returns The matching descendants.
   */
  querySelectorAll(selectors: string): NodeListOf<Element>;
}
