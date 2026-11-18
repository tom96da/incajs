// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { Event, GlobalEventHandlersEventMap } from "../types.mts";
import type { HANDLER_EVENTS } from "./eventHandlers.mts";
import type { DOMRect } from "./geometry.mts";
import type {
  CharacterData,
  ChildNode,
  Node,
  NonDocumentTypeChildNode,
  ParentNode,
  TextMembers,
} from "./nodeTypes.mts";
import type { ScrollIntoViewOptions, ScrollOptions } from "./scroll.mts";

/** A function an event handler property holds, called with its target as `this` and the event. */
export type EventHandler<E = Event, T = Element> = (this: T, event: E) => unknown;

/**
 * The event handler properties, such as `onclick`, of the events the host dispatches, `focusin` and
 * `focusout` excepted.
 */
export type EventHandlers<T = Element> = {
  /**
   * The handler of the event: a function, an object kept as assigned, or `null`. Assigning a
   * function or an object sets it, and any other value sets `null`. A value assigned while the
   * property holds `null` registers a listener after the target's other listeners. A later value
   * keeps that place, and `null` removes it. An object runs nothing. A handler that returns `false`
   * cancels the event. On an element, assigning does nothing after the node is removed.
   */
  [K in (typeof HANDLER_EVENTS)[number] as `on${K}`]:
    | EventHandler<GlobalEventHandlersEventMap[K], T>
    | object
    | null;
};

/** A live, read-only list of elements. */
export interface HTMLCollectionOf<T extends Element> {
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
export interface NodeListOf<T extends Node> {
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
  /** The attribute name as the element keys it: lower case on an HTML element. */
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
  readonly ownerElement: Element | null;
}

/**
 * The `data-*` attributes of an element as properties. The key `fooBar` names the attribute
 * `data-foo-bar`. Reads give text and writes convert the value to text.
 */
export interface DOMStringMap {
  [name: string]: string | undefined;
}

/** The live set of the tokens of the `class` attribute. It is read from `classList`. */
export interface DOMTokenList {
  /** The number of tokens. */
  readonly length: number;
  /** The attribute text. Assigning sets the attribute. */
  value: string;
  toString(): string;
  /** Returns the token at `index`, or `null`. */
  item(index: number): string | null;
  contains(token: string): boolean;
  add(...tokens: string[]): void;
  remove(...tokens: string[]): void;
  toggle(token: string, force?: boolean): boolean;
  replace(token: string, newToken: string): boolean;
  /** Throws a `TypeError`, because `class` defines no supported tokens. */
  supports(token: string): boolean;
  forEach(
    callback: (token: string, index: number, list: DOMTokenList) => void,
    thisArg?: unknown,
  ): void;
  keys(): IteratorObject<number, undefined, unknown>;
  values(): IteratorObject<string, undefined, unknown>;
  entries(): IteratorObject<[number, string], undefined, unknown>;
  [Symbol.iterator](): IteratorObject<string, undefined, unknown>;
}

/** A live, read-only list of the attributes of an element in the order they were set. */
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

/**
 * The inline style of an element. Every style key and every CSS alias is a property in camelCase
 * and in kebab-case, and the host-only style keys are properties too. Reading a property gives the
 * declaration in its serialized form, or an empty string when the block has none.
 * Assigning a property sets the declaration, and an empty string removes it. A value the host
 * cannot read prints a warning and leaves the block as it was. The block of a freed element reads
 * empty and ignores writes. It is not constructible, has no indexed access and is not iterable.
 */
export interface CSSStyleDeclaration {
  /** The number of declarations. */
  readonly length: number;
  /**
   * The declarations as `name: value;` text in the order they were set. Assigning replaces all
   * declarations with the ones the text holds, and `null` assigns `""`.
   */
  cssText: string;
  /**
   * The serialized value of the declaration `property`, or an empty string when it is unset.
   * @param property - The CSS name, such as `background-color`.
   * @returns The value.
   */
  getPropertyValue(property: string): string;
  /**
   * Sets the declaration `property` to `value`, and removes it when `value` is `null` or `""`.
   * A `priority` other than `""` and `"important"` leaves the block as it was.
   * @param property - The CSS name.
   * @param value - The value, converted to text.
   * @param priority - The priority, which the host does not keep.
   */
  setProperty(property: string, value: string | null, priority?: string | null): void;
  /**
   * Removes the declaration `property`.
   * @param property - The CSS name.
   * @returns The value the declaration had, or an empty string when it was unset.
   */
  removeProperty(property: string): string;
  /**
   * The priority of the declaration `property`.
   * @param property - The CSS name.
   * @returns Always an empty string.
   */
  getPropertyPriority(property: string): string;
  /**
   * The CSS name of the declaration at `index`, in the order they were set.
   * @param index - The position, converted to an unsigned 32-bit integer.
   * @returns The name, or an empty string beyond the list.
   */
  item(index: number): string;
  display: string;
  "flex-direction": string;
  flexDirection: string;
  "justify-content": string;
  justifyContent: string;
  "align-items": string;
  alignItems: string;
  gap: string;
  width: string;
  height: string;
  "border-width": string;
  borderWidth: string;
  background: string;
  "background-color": string;
  backgroundColor: string;
  "border-color": string;
  borderColor: string;
  "corner-radius": string;
  cornerRadius: string;
  "border-radius": string;
  borderRadius: string;
  "text-color": string;
  textColor: string;
  color: string;
  "text-size": string;
  textSize: string;
  "font-size": string;
  fontSize: string;
  overflow: string;
  "overflow-x": string;
  overflowX: string;
  "overflow-y": string;
  overflowY: string;
  "flex-grow": string;
  flexGrow: string;
  "flex-shrink": string;
  flexShrink: string;
  opacity: string;
  "min-width": string;
  minWidth: string;
  "min-height": string;
  minHeight: string;
  "max-width": string;
  maxWidth: string;
  "max-height": string;
  maxHeight: string;
  padding: string;
  "padding-x": string;
  paddingX: string;
  "padding-y": string;
  paddingY: string;
  "padding-top": string;
  paddingTop: string;
  "padding-right": string;
  paddingRight: string;
  "padding-bottom": string;
  paddingBottom: string;
  "padding-left": string;
  paddingLeft: string;
  margin: string;
  "margin-x": string;
  marginX: string;
  "margin-y": string;
  marginY: string;
  "margin-top": string;
  marginTop: string;
  "margin-right": string;
  marginRight: string;
  "margin-bottom": string;
  marginBottom: string;
  "margin-left": string;
  marginLeft: string;
}

/**
 * The properties that reflect `role` and the `aria-*` attributes. A text property reads the
 * attribute, or `null` when it is absent. Assigning `null` or `undefined` removes the attribute
 * and any other value sets its text. The element properties read the elements an attribute names
 * by ID, or the elements assigned. An assignment sets the attribute to an empty string. Nothing
 * reaches assistive technology.
 */
export interface ARIAMixin {
  role: string | null;
  ariaActiveDescendantElement: Element | null;
  ariaAtomic: string | null;
  ariaAutoComplete: string | null;
  ariaBrailleLabel: string | null;
  ariaBrailleRoleDescription: string | null;
  ariaBusy: string | null;
  ariaChecked: string | null;
  ariaColCount: string | null;
  ariaColIndex: string | null;
  ariaColIndexText: string | null;
  ariaColSpan: string | null;
  ariaControlsElements: readonly Element[] | null;
  ariaCurrent: string | null;
  ariaDescribedByElements: readonly Element[] | null;
  ariaDescription: string | null;
  ariaDetailsElements: readonly Element[] | null;
  ariaDisabled: string | null;
  ariaErrorMessageElements: readonly Element[] | null;
  ariaExpanded: string | null;
  ariaFlowToElements: readonly Element[] | null;
  ariaHasPopup: string | null;
  ariaHidden: string | null;
  ariaInvalid: string | null;
  ariaKeyShortcuts: string | null;
  ariaLabel: string | null;
  ariaLabelledByElements: readonly Element[] | null;
  ariaLevel: string | null;
  ariaLive: string | null;
  ariaModal: string | null;
  ariaMultiLine: string | null;
  ariaMultiSelectable: string | null;
  ariaOrientation: string | null;
  ariaOwnsElements: readonly Element[] | null;
  ariaPlaceholder: string | null;
  ariaPosInSet: string | null;
  ariaPressed: string | null;
  ariaReadOnly: string | null;
  ariaRelevant: string | null;
  ariaRequired: string | null;
  ariaRoleDescription: string | null;
  ariaRowCount: string | null;
  ariaRowIndex: string | null;
  ariaRowIndexText: string | null;
  ariaRowSpan: string | null;
  ariaSelected: string | null;
  ariaSetSize: string | null;
  ariaSort: string | null;
  ariaValueMax: string | null;
  ariaValueMin: string | null;
  ariaValueNow: string | null;
  ariaValueText: string | null;
}

/** The members that read and write the markup of an element. */
export interface HtmlMembers {
  /**
   * The markup of the children. Assigning replaces all children with the nodes the string parses
   * to in the context of the element, and `null` assigns `""`. A template reads and replaces its
   * content.
   */
  innerHTML: string;
  /**
   * The markup of the element and its children. Assigning replaces the element with the nodes the
   * string parses to in the context of its parent, and `null` assigns `""`. An element without a
   * parent keeps its place. An element whose parent is `document` throws a
   * `NoModificationAllowedError`.
   */
  outerHTML: string;
  /**
   * Parses `text` and inserts the nodes at `position`: `beforebegin`, `afterbegin`, `beforeend` or
   * `afterend`, in any ASCII case. Another position throws a `SyntaxError`. `beforebegin` and
   * `afterend` on an element without a parent or whose parent is `document` throw a
   * `NoModificationAllowedError`.
   */
  insertAdjacentHTML(position: string, text: string): void;
}

/**
 * An element, a node with a tag, attributes and children.
 *
 * A removed node keeps its attributes and its position in its own subtree, and its members read
 * them. Its layout reads give `0`. Its listeners are dropped. `addEventListener()` and
 * `removeEventListener()` ignore the call, an assignment to a handler property does nothing, and
 * `dispatchEvent()` runs no listener on it.
 */
export interface Element
  extends ParentNode, ChildNode, NonDocumentTypeChildNode, HtmlMembers, ARIAMixin {
  readonly nodeType: 1;
  /**
   * The `id` attribute, or an empty string when it is unset. Assigning sets
   * the attribute to the value as a string.
   */
  id: string;
  /**
   * The `class` attribute, or an empty string when it is unset. Assigning sets the attribute to the
   * value as a string.
   */
  className: string;
  /**
   * The live token list of the `class` attribute. Every read returns the same list. Assigning a
   * string sets the `value` of the list.
   */
  get classList(): DOMTokenList;
  set classList(value: string);
  /** @internal */
  readonly kind: "element";
  /**
   * The qualified name of the element. An element of the HTML namespace reads in ASCII upper case,
   * such as `"BUTTON"`, and the mount container reads `"DIV"`.
   */
  readonly tagName: string;
  /** The name of the element without its prefix. */
  readonly localName: string;
  /** The namespace of the element, or `null` for none. */
  readonly namespaceURI: string | null;
  /** The prefix of the element name, or `null` for none. */
  readonly prefix: string | null;
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
  /** The names of the set attributes in the order they were set. */
  getAttributeNames(): string[];
  /**
   * The set attributes as a live map. Every read returns the same object, which follows later
   * changes. A key that equals the name of a set attribute reads its `Attr`.
   */
  readonly attributes: NamedNodeMap;
  /**
   * Whether this element matches the selector list. `:focus-visible` and the other interactive and
   * form-state pseudo-classes never match.
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
  closest(selectors: string): Element | null;
  /**
   * The box this node was drawn at in the last frame, in window logical
   * pixels. A node created in the same task reads all zeros.
   */
  getBoundingClientRect(): DOMRect;
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

/**
 * An element of the HTML namespace: an element with the members of HTML elements, the shared
 * element members, `click()` and the offset members.
 */
export interface HTMLElement extends Element, EventHandlers<HTMLElement> {
  /**
   * Focuses this node next frame. The node needs a `tabindex` attribute. A
   * `true` `preventScroll` keeps every scroll container where it is. After
   * the node is removed, the call does nothing.
   */
  focus(options?: { preventScroll?: boolean }): void;
  /** Unfocuses this node next frame, if it is the focused one. */
  blur(): void;
  /**
   * Assigning sets the `tabindex` attribute to the value's integer part,
   * wrapped to 32 bits. Reading returns the `tabindex` attribute as an
   * integer, or the default of the element kind when it is unset or invalid: `0` for a `button`, an
   * `input`, a link and the other kinds the HTML standard lists, `-1` for the rest.
   */
  tabIndex: number;
  /**
   * The inline style as a live block. Every read returns the same object. Assigning a string
   * writes its `cssText`, and `null` assigns `""`.
   */
  get style(): CSSStyleDeclaration;
  set style(cssText: string | null);
  /**
   * The `data-*` attributes as a live object. Every read returns the same object. A write converts
   * the value to text.
   * @throws DOMException named `SyntaxError` for a write to a key with a dash before a lower case
   * letter, and named `InvalidCharacterError` for a key that makes an invalid attribute name
   * @throws TypeError for a write of a `Symbol` value
   */
  readonly dataset: DOMStringMap;
  /**
   * Fires an untrusted `click` at this node. A disabled `button` or `input` and a node
   * whose own click is still running ignore the call. After the node is removed, the call
   * does nothing.
   */
  click(): void;
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
  readonly offsetParent: Element | null;
}

/** The handle of an `<input>`. */
export interface HTMLInputElement extends HTMLElement {
  /**
   * The text of the input. Assigning sets the value as a string, and `null` as `""`; line breaks
   * are dropped and a changed text puts the caret at its end.
   */
  value: string;
}

/** The handle of a `<template>`. */
export interface HTMLTemplateElement extends HTMLElement {
  /** The fragment that holds the markup of the template. It is never drawn or connected. */
  readonly content: DocumentFragment;
}

/** A comment node. */
export interface Comment extends CharacterData, ChildNode, NonDocumentTypeChildNode {
  readonly nodeType: 8;
  /** @internal */
  readonly kind: "comment";
}

/** A text leaf node. */
export interface Text extends CharacterData, ChildNode, NonDocumentTypeChildNode, TextMembers {
  readonly nodeType: 3;
  /** @internal */
  readonly kind: "text";
}

/**
 * The node `document.createDocumentFragment()` returns: a parentless container whose children move
 * into the node it is inserted into.
 */
export interface DocumentFragment extends ParentNode {
  readonly nodeType: 11;
  /** @internal */
  readonly kind: "fragment";
}
