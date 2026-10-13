// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { IncaEventTarget, NodeId } from "../rendererCore.mts";
import type { RectLike } from "./geometry.mts";
import type { ScrollIntoViewOptions, ScrollOptions } from "./scroll.mts";

/**
 * A container node: either a real element or the hidden stand-in that
 * `createCommentNode` makes. It is the only node type that can act as a
 * parent.
 *
 * `inca`'s native tree has no parent pointers and no way to list a
 * node's children, so `parent`/`children` are maintained here as a
 * JS-side shadow of the tree, kept in sync by `insert` and
 * `remove`.
 *
 * After the node is removed, `addEventListener()` and `removeEventListener()`
 * ignore the call, and `dispatchEvent()` returns `true`.
 */
export interface IncaElement extends IncaEventTarget {
  /**
   * The underlying native node's id, the only part of this object the native
   * tree itself knows about.
   */
  readonly id: NodeId;
  /**
   * `"comment"` for the hidden stand-in `createCommentNode`
   * makes; `"element"` for every other container. Purely
   * informational: both render identically once created.
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
   * Fires an untrusted `click` at this node. A disabled `button`, a node whose
   * own click is still running and an unknown node ignore the call.
   */
  click(): void;
  /**
   * Assigning sets the `tabindex` attribute to the value's integer part,
   * wrapped to 32 bits. Reading returns the `tabindex` attribute as an
   * integer, or `-1` (`0` for a `button`) when it is unset or invalid.
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

/** A text leaf node. */
export interface IncaText {
  /** The underlying native node's id. */
  readonly id: NodeId;
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
