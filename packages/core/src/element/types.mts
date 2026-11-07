// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { IncaDocument, IncaEvent, IncaEventMap, IncaEventTarget, NodeId } from "../types.mts";
import type { HANDLER_EVENTS } from "./eventHandlers.mts";
import type { RectLike } from "./geometry.mts";
import type { ScrollIntoViewOptions, ScrollOptions } from "./scroll.mts";

/** A function an event handler property holds, called with its target as `this` and the event. */
export type EventHandler<E = IncaEvent, T = IncaElement> = (this: T, event: E) => unknown;

/**
 * The event handler properties, such as `onclick`, of the events the host dispatches, `focusin` and
 * `focusout` excepted.
 */
export type EventHandlers<T = IncaElement> = {
  /**
   * The handler of the event: a function, an object kept as assigned, or `null`. Assigning a
   * function or an object sets it, and any other value sets `null`. A value assigned while the
   * property holds `null` registers a listener after the target's other listeners. A later value
   * keeps that place, and `null` removes it. An object runs nothing. A handler that returns `false`
   * cancels the event. On an element, assigning does nothing after the node is removed.
   */
  [K in (typeof HANDLER_EVENTS)[number] as `on${K}`]:
    | EventHandler<IncaEventMap[K], T>
    | object
    | null;
};

/** A live, read-only list of elements. */
export interface HTMLCollectionOf<T extends IncaElement> {
  readonly [index: number]: T;
  /** The number of elements in the list. */
  readonly length: number;
  /**
   * The element at `index`, or `null` beyond the list. The index converts to an unsigned 32-bit
   * integer.
   * @param index - The position in the list.
   * @returns The element at `index`, or `null`.
   */
  item(index: number): T | null;
  /**
   * The first element whose `id` is `name`, else the first whose `name` attribute is `name`, or
   * `null`. The empty string finds none.
   * @param name - An `id` or `name` attribute value.
   * @returns The matching element, or `null`.
   */
  namedItem(name: string): T | null;
  [Symbol.iterator](): IterableIterator<T>;
}

/**
 * A read-only list of nodes. The list of `childNodes` follows later changes. The list of
 * `querySelectorAll()` holds the elements found when it was made.
 */
export interface NodeListOf<T extends IncaNode> {
  readonly [index: number]: T;
  /** The number of nodes in the list. */
  readonly length: number;
  /**
   * The node at `index`, or `null` beyond the list. The index converts to an unsigned 32-bit
   * integer.
   * @param index - The position in the list.
   * @returns The node at `index`, or `null`.
   */
  item(index: number): T | null;
  /**
   * Calls `callback` with the node, its index and the list for each node in order.
   * @throws TypeError when `callback` is not a function
   * @param callback - The function to call for each node.
   * @param thisArg - The value of `this` inside `callback`.
   */
  forEach(
    callback: (value: T, index: number, list: NodeListOf<T>) => void,
    thisArg?: unknown,
  ): void;
  /** The pairs of index and node in order. */
  entries(): IterableIterator<[number, T]>;
  /** The indices in order. */
  keys(): IterableIterator<number>;
  /** The nodes in order. */
  values(): IterableIterator<T>;
  [Symbol.iterator](): IterableIterator<T>;
}

/** An attribute of an element. It has no `Node` members. */
export interface Attr {
  /** The attribute name in lower case. */
  readonly name: string;
  /** The same as `name`. */
  readonly localName: string;
  /**
   * The value. While the element holds the attribute, a read gives its current value and an
   * assignment sets it as a string. After the attribute is removed, the `Attr` keeps the last
   * value it read, and an assignment changes the `Attr` only.
   */
  value: string;
  /** Always `null`. */
  readonly namespaceURI: null;
  /** Always `null`. */
  readonly prefix: null;
  /** Always `true`. */
  readonly specified: true;
  /** The element that holds the attribute, or `null` once the attribute is removed. */
  readonly ownerElement: IncaElement | null;
}

/** A live, read-only list of the attributes of an element in alphabetical order of the name. */
export interface NamedNodeMap {
  readonly [index: number]: Attr;
  /** The number of attributes. */
  readonly length: number;
  /**
   * The attribute at `index`, or `null` beyond the list. The index converts to an unsigned 32-bit
   * integer.
   * @param index - The position in the list.
   * @returns The attribute at `index`, or `null`.
   */
  item(index: number): Attr | null;
  /**
   * The attribute named `qualifiedName`, or `null`. The letters `A` to `Z` of the name match their
   * lower case. Reading the same attribute twice gives the same object while it stays set.
   * @param qualifiedName - The attribute name.
   * @returns The attribute, or `null`.
   */
  getNamedItem(qualifiedName: string): Attr | null;
  [Symbol.iterator](): IterableIterator<Attr>;
}

/** The members every node has: what it is, its data and its place in the tree. */
export interface NodeMembers {
  /**
   * `1` for an element, `3` for a text node, `8` for a comment stand-in and `11` for a
   * fragment.
   */
  readonly nodeType: 1 | 3 | 8 | 11;
  /**
   * The {@link IncaElement.tagName} of an element, `"#text"` for a text node, `"#comment"` for a
   * comment stand-in and `"#document-fragment"` for a fragment.
   */
  readonly nodeName: string;
  /**
   * The data of a text node or a comment stand-in, and `null` for an element or a
   * fragment. Assigning `null` or `undefined` stores `""` and any other value stores its string.
   * Assigning to an element, a fragment or a freed node does nothing.
   */
  nodeValue: string | null;
  /** Whether the node hangs under the mount container, which counts as connected. */
  readonly isConnected: boolean;
  /** The `document` export. */
  readonly ownerDocument: IncaDocument;
  /**
   * The parent, `document` for the mount container and `null` for a detached node. A node inside
   * a fragment reads the fragment. A freed node reads `null`.
   */
  readonly parentNode: IncaElement | IncaFragment | IncaDocument | null;
  /** The first child, or `null`. */
  readonly firstChild: IncaNode | null;
  /** The last child, or `null`. */
  readonly lastChild: IncaNode | null;
  /** The sibling right before this node, or `null`. */
  readonly previousSibling: IncaNode | null;
  /** The sibling right after this node, or `null`. */
  readonly nextSibling: IncaNode | null;
  /**
   * The children in order, as a live list. Every read returns the same object, which follows later
   * changes. A freed node has an empty list.
   */
  readonly childNodes: NodeListOf<IncaNode>;
  /** Whether the node has a child. */
  hasChildNodes(): boolean;
  /**
   * Appends `node` as the last child. A fragment gives up its children in order and stays empty.
   * A node that has a parent moves. Throws `HierarchyRequestError` when this node is a text node or
   * a comment, or when `node` is this node or one of its ancestors.
   * @returns `node`
   */
  appendChild<T extends IncaNode>(node: T): T;
  /**
   * Inserts `node` before `child`, or last when `child` is `null` or `undefined`. A fragment gives
   * up its children in order and stays empty. Throws `HierarchyRequestError` as
   * {@link NodeMembers.appendChild} does, and `NotFoundError` when `child` is not a child of this
   * node.
   * @returns `node`
   */
  insertBefore<T extends IncaNode>(node: T, child: IncaNode | null | undefined): T;
  /**
   * Removes `child`, which keeps its listeners and can be inserted again. Throws `NotFoundError`
   * when `child` is not a child of this node.
   * @returns `child`
   */
  removeChild<T extends IncaNode>(child: T): T;
  /**
   * Puts `node` in the place of `child`, which keeps its listeners and can be inserted again.
   * Throws as {@link NodeMembers.insertBefore} does, with `child` as the node that has to be a
   * child of this node.
   * @returns `child`
   */
  replaceChild<T extends IncaNode>(node: IncaNode, child: T): T;
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
  contains(other: IncaNode | null): boolean;
  /**
   * The `document` export while the node hangs under the mount container, else the topmost node
   * of its subtree, the node itself without a parent.
   */
  getRootNode(): IncaNode | IncaDocument;
  /**
   * The position of `other` relative to this node as a bit mask: `1` disconnected, `2` preceding,
   * `4` following, `8` contains, `16` contained by and `32` implementation specific. Nodes of
   * different trees give `1 | 32` plus `2` or `4`, with opposite directions for the two orders of
   * the pair. A freed node is a tree of its own: it gives `1 | 32` plus `2` or `4` by the order of
   * the two ids, and `0` for itself.
   * @throws {TypeError} when `other` is not a node
   */
  compareDocumentPosition(other: IncaNode): number;
  /** Whether `other` is this node. `null` and `undefined` give `false`. */
  isSameNode(other: IncaNode | null): boolean;
  /**
   * Whether `other` has the same tag, attributes, style and children, compared in depth. The typed
   * text of an `input` is not compared. `null` and `undefined` give `false`. A freed node equals
   * only itself.
   */
  isEqualNode(other: IncaNode | null): boolean;
}

/** The `textContent` of a node. */
export interface TextContentMember {
  /**
   * The data of a text node or a comment stand-in, and the data of the text nodes below an
   * element in tree order, comment stand-ins left out. Assigning `null` or `undefined` stores `""`
   * and any other value stores its string. Assigning to an element replaces its children with one
   * text node, or with none for `""`. A freed node reads `""` or its last data and ignores the
   * assignment.
   */
  textContent: string;
}

/** The members of a text node and a comment stand-in that read and edit their data. */
export interface CharacterDataMembers extends TextContentMember {
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
  splitText(offset: number): IncaText;
}

/**
 * A container node: either a real element or the hidden stand-in that
 * `createCommentNode` makes. It is the only node type that can act as a
 * parent. `parent` reads the node's current position in the tree.
 *
 * A removed node keeps its attributes and its position in its own subtree, and its members read
 * them. Its listeners are dropped. `addEventListener()` and `removeEventListener()` ignore the
 * call, an assignment to a handler property does nothing, and `dispatchEvent()` runs no listener
 * on it.
 */
export interface IncaElement
  extends IncaEventTarget, EventHandlers, NodeMembers, TextContentMember {
  /**
   * The node id the host knows this node by. Internal to the renderer.
   * @internal
   */
  readonly __node_id: NodeId;
  /**
   * The `id` attribute, or an empty string when it is unset. Assigning sets
   * the attribute to the value as a string.
   */
  id: string;
  /**
   * `"comment"` for the hidden stand-in `createCommentNode` makes;
   * `"element"` for every other container.
   */
  readonly kind: "element" | "comment";
  /**
   * The tag of the element in ASCII upper case, such as `"BUTTON"`. The mount container reads
   * `"DIV"`.
   */
  readonly tagName: string;
  /** The current parent element or fragment, or `null` if this node isn't attached to the tree. */
  parent: IncaParent | null;
  /**
   * The children of this node in render order, text leaves included.
   * @internal
   */
  __children: IncaNode[];
  /**
   * The child elements in order, as a live collection. Every read returns the same object, which
   * follows later changes. A removed element lists the element children it kept.
   */
  readonly children: HTMLCollectionOf<IncaElement>;
  /**
   * The parent element, or `null` for a detached node, a node inside a fragment and the mount
   * container.
   */
  readonly parentElement: IncaElement | null;
  /** The first child that is an element, or `null`. */
  readonly firstElementChild: IncaElement | null;
  /** The last child that is an element, or `null`. */
  readonly lastElementChild: IncaElement | null;
  /** The number of children that are elements. */
  readonly childElementCount: number;
  /** The closest earlier sibling that is an element, or `null`. */
  readonly previousElementSibling: IncaElement | null;
  /** The closest later sibling that is an element, or `null`. */
  readonly nextElementSibling: IncaElement | null;
  /**
   * Focuses this node next frame. The node needs a `tabindex` attribute. A
   * `true` `preventScroll` keeps every scroll container where it is. After
   * the node is removed, the call does nothing.
   */
  focus(options?: { preventScroll?: boolean }): void;
  /** Unfocuses this node next frame, if it is the focused one. */
  blur(): void;
  /**
   * Fires an untrusted `click` at this node. A disabled `button` or `input` and a node
   * whose own click is still running ignore the call. After the node is removed, the call
   * does nothing.
   */
  click(): void;
  /**
   * Captures the mouse pointer for this node while a button is held. Only `pointerId` 1 exists.
   * @throws DOMException named `NotFoundError` for another `pointerId`
   * @throws DOMException named `InvalidStateError` when the node is outside the mounted tree,
   * also after it is removed
   */
  setPointerCapture(pointerId: number): void;
  /**
   * Releases the capture this node requested. Does nothing when the node holds none.
   * @throws DOMException named `NotFoundError` for a `pointerId` other than 1
   */
  releasePointerCapture(pointerId: number): void;
  /**
   * Whether this node requested the capture and has not released it. `false` after the node is
   * removed.
   */
  hasPointerCapture(pointerId: number): boolean;
  /**
   * Assigning sets the `tabindex` attribute to the value's integer part,
   * wrapped to 32 bits. Reading returns the `tabindex` attribute as an
   * integer, or `-1` (`0` for a `button` or an `input`) when it is unset or invalid.
   */
  tabIndex: number;
  /**
   * The value of the attribute `name` as a string, or `null` when it is unset. Names are
   * ASCII-case-insensitive.
   * @param name - The attribute name.
   * @returns The attribute value, or `null` when the attribute is unset.
   */
  getAttribute(name: string): string | null;
  /**
   * Sets the attribute `name` to `String(value)`. Names are ASCII-case-insensitive.
   * `disabled`, `hidden`, `inert` and `readonly` take effect by presence.
   * @throws DOMException named `InvalidCharacterError` for an empty name, or one that holds ASCII
   * whitespace, NUL, `/`, `=` or `>`
   * @param name - The attribute name.
   * @param value - The new value, converted with `String`.
   */
  setAttribute(name: string, value: string): void;
  /**
   * Whether the attribute `name` is set, an empty value included.
   * @param name - The attribute name.
   * @returns `true` when the attribute is set.
   */
  hasAttribute(name: string): boolean;
  /**
   * Removes the attribute `name`. An unset name changes nothing.
   * @param name - The attribute name.
   */
  removeAttribute(name: string): void;
  /** The names of the set attributes in alphabetical order. */
  getAttributeNames(): string[];
  /**
   * The set attributes as a live map. Every read returns the same object, which follows later
   * changes. A key that equals the name of a set attribute reads its `Attr`.
   */
  readonly attributes: NamedNodeMap;
  /**
   * Whether this element matches the selector list. `:hover`, `:focus` and the other interactive
   * and form-state pseudo-classes never match.
   * @throws DOMException named `SyntaxError` for an invalid selector, also after this element is
   * removed
   * @param selectors - A selector list.
   * @returns `true` when this element matches the selector list.
   */
  matches(selectors: string): boolean;
  /**
   * The nearest element at or above this one that matches the selector list, or `null`.
   * @throws DOMException named `SyntaxError` for an invalid selector, also after this element is
   * removed
   * @param selectors - A selector list.
   * @returns The matching element, or `null` when none matches.
   */
  closest(selectors: string): IncaElement | null;
  /**
   * The first descendant element that matches the selector list, or `null`. This element is never
   * in the result. `:scope` and a leading `&` select this element. A removed element searches the
   * subtree it kept.
   * @throws DOMException named `SyntaxError` for an invalid selector, also after this element is
   * removed
   * @param selectors - A selector list.
   * @returns The first matching descendant in document order, or `null`.
   */
  querySelector(selectors: string): IncaElement | null;
  /**
   * The descendant elements that match the selector list, in document order, as a list that later
   * changes do not reach. Every call returns a new list. A removed element searches the subtree it
   * kept.
   * @throws DOMException named `SyntaxError` for an invalid selector, also after this element is
   * removed
   * @param selectors - A selector list.
   * @returns The matching descendants.
   */
  querySelectorAll(selectors: string): NodeListOf<IncaElement>;
  /**
   * The box this node was drawn at in the last frame, in window logical
   * pixels. A node created in the same task reads all zeros.
   */
  getBoundingClientRect(): RectLike;
  /** The box width from the last frame, rounded. `0` until a frame draws the node. */
  readonly offsetWidth: number;
  /** The box height from the last frame, rounded. `0` until a frame draws the node. */
  readonly offsetHeight: number;
  /** The box's left edge from the last frame, relative to {@link offsetParent}. Stays fixed while the parent scrolls. */
  readonly offsetLeft: number;
  /** The box's top edge from the last frame, relative to {@link offsetParent}. Stays fixed while the parent scrolls. */
  readonly offsetTop: number;
  /**
   * The parent, when this node and the parent both have a box from the last frame.
   * `null` when either lacks a box or there is no parent.
   */
  readonly offsetParent: IncaElement | null;
  /** The box width less both borders, rounded, at least `0`. */
  readonly clientWidth: number;
  /** The box height less both borders, rounded, at least `0`. */
  readonly clientHeight: number;
  /** The border width, rounded. */
  readonly clientLeft: number;
  /** The border width, rounded. */
  readonly clientTop: number;
  /** The horizontal scroll offset. Assigning moves it at once, and reads are clamped to the last frame's maximum. */
  scrollLeft: number;
  /** The vertical scroll offset. Assigning moves it at once, and reads are clamped to the last frame's maximum. */
  scrollTop: number;
  /** The content width: {@link clientWidth} plus the largest horizontal offset. */
  readonly scrollWidth: number;
  /** The content height: {@link clientHeight} plus the largest vertical offset. */
  readonly scrollHeight: number;
  /** Sets the offset to `(left, top)`, or to the options object's `left` and `top`. */
  scrollTo(left?: number, top?: number): void;
  scrollTo(options: ScrollOptions): void;
  /** Adds `(left, top)` to the offset, or the options object's `left` and `top`. */
  scrollBy(left?: number, top?: number): void;
  scrollBy(options: ScrollOptions): void;
  /**
   * Scrolls each scroll container around this node so the node shows. `true`
   * or no argument aligns to the start and `false` to the end. An options
   * object sets `block` (default `"start"`) and `inline` (default `"nearest"`);
   * `behavior` is accepted and ignored. After the node is removed, the call
   * does nothing.
   */
  scrollIntoView(arg?: boolean | ScrollIntoViewOptions): void;
}

/** The handle of an `<input>`. */
export interface IncaInputElement extends IncaElement {
  /**
   * The text of the input. Assigning sets the value as a string, and `null` as `""`; line breaks
   * are dropped and a changed text puts the caret at its end.
   */
  value: string;
}

/** A text leaf node. */
export interface IncaText extends NodeMembers, CharacterDataMembers, TextMembers {
  /**
   * The node id the host knows this node by. Internal to the renderer.
   * @internal
   */
  readonly __node_id: NodeId;
  /** Discriminates this node within the {@link IncaNode} union. */
  readonly kind: "text";
  /** The current parent element or fragment, or `null` if this node isn't attached to the tree. */
  parent: IncaParent | null;
  /**
   * The text content, mirroring the value pushed to the native node.
   */
  text: string;
}

/**
 * The node `document.createDocumentFragment()` returns: a parentless container whose children move
 * into the node it is inserted into.
 */
export interface IncaFragment extends NodeMembers, TextContentMember {
  /**
   * The node id the host knows this node by. Internal to the renderer.
   * @internal
   */
  readonly __node_id: NodeId;
  /** Discriminates this node within the {@link IncaNode} union. */
  readonly kind: "fragment";
  /** A fragment has no parent. */
  parent: null;
  /**
   * The children of this node in order, text leaves included.
   * @internal
   */
  __children: IncaNode[];
}

/** A node that can have children. */
export type IncaParent = IncaElement | IncaFragment;

/** Any node the shadow tree holds: an element, comment, text leaf or fragment. */
export type IncaNode = IncaElement | IncaText | IncaFragment;
