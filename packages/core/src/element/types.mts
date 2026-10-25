// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { IncaEvent, IncaEventMap, IncaEventTarget, NodeId } from "../types.mts";
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

/**
 * A container node: either a real element or the hidden stand-in that
 * `createCommentNode` makes. It is the only node type that can act as a
 * parent. `parent` and `children` read the node's current position in the
 * tree.
 *
 * After the node is removed, `addEventListener()` and `removeEventListener()`
 * ignore the call, an assignment to a handler property does nothing, and
 * `dispatchEvent()` returns `true`.
 */
export interface IncaElement extends IncaEventTarget, EventHandlers {
  /**
   * The node id the host knows this node by. Internal to the renderer.
   * @internal
   */
  readonly __node_id: NodeId;
  /**
   * The `id` attribute, or an empty string when it is unset. Assigning sets
   * the attribute to the value as a string. After the node is removed,
   * reading gives an empty string and assigning does nothing.
   */
  id: string;
  /**
   * `"comment"` for the hidden stand-in `createCommentNode` makes;
   * `"element"` for every other container.
   */
  readonly kind: "element" | "comment";
  /** The current parent, or `null` if this node isn't attached to the tree. */
  parent: IncaElement | null;
  /** This node's children, in render order. */
  children: IncaNode[];
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
   * Whether `other` is this node or one of its descendants in the tree as
   * rendered. `null` and `undefined` give `false`, and any other value that is
   * not a node throws a `TypeError`. After the node is removed, it still answers
   * for the subtree it keeps.
   */
  contains(other: IncaNode | null): boolean;
  /**
   * Assigning sets the `tabindex` attribute to the value's integer part,
   * wrapped to 32 bits. Reading returns the `tabindex` attribute as an
   * integer, or `-1` (`0` for a `button` or an `input`) when it is unset or invalid.
   * After the node is removed, reading gives that default and assigning does
   * nothing.
   */
  tabIndex: number;
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
   * are dropped and a changed text puts the caret at its end. After the node is removed, reading
   * gives `""` and assigning does nothing.
   */
  value: string;
}

/** A text leaf node. */
export interface IncaText {
  /**
   * The node id the host knows this node by. Internal to the renderer.
   * @internal
   */
  readonly __node_id: NodeId;
  /** Discriminates this node within the {@link IncaNode} union. */
  readonly kind: "text";
  /** The current parent, or `null` if this node isn't attached to the tree. */
  parent: IncaElement | null;
  /**
   * The text content, mirroring the value pushed to the native node.
   */
  text: string;
}

/** Any node the shadow tree holds: an element, comment, or text leaf. */
export type IncaNode = IncaElement | IncaText;
