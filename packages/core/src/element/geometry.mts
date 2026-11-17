// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { layoutOf, linkedNode } from "../ffi/index.mts";
import { ELEMENT_RULE, HTML_ELEMENT_RULE, isRemoved, receiverNode } from "./lifecycle.mts";
import type { NodeBox } from "../ffi/index.mts";
import type { HandleBuilders, Receiver } from "./lifecycle.mts";
import type { Element, HTMLElement } from "./types.mts";

/** The members {@link layoutMembers} builds. */
export type LayoutMembers = Pick<
  Element,
  "getBoundingClientRect" | "clientWidth" | "clientHeight" | "clientLeft" | "clientTop"
>;

/** The members {@link offsetMembers} builds. */
export type OffsetMembers = Pick<
  HTMLElement,
  "offsetWidth" | "offsetHeight" | "offsetLeft" | "offsetTop" | "offsetParent"
>;

/**
 * Reads the box a node was last drawn at. A removed node, or one with no
 * recorded box, yields `null`.
 * @param el - the node to read
 * @returns the box, or `null`
 */
export function layoutBox(el: Element): NodeBox | null {
  return isRemoved(el) ? null : layoutOf(el.__node_id);
}

/**
 * The parent a node's offsets are measured from. It is the parent element when both
 * the node and the parent have a recorded box.
 * @param el - the node to query
 * @param builders - the lookup of a handle by node id
 * @returns the parent, or `null`
 */
export function offsetParentOf(el: Element, builders: HandleBuilders): Element | null {
  if (!layoutBox(el)) return null;
  const id = linkedNode(el.__node_id, "parent", true);
  const parent = id === null ? undefined : (builders.node(id) as Element | undefined);
  return parent && layoutBox(parent) ? parent : null;
}

/**
 * Where a node sits inside the padding edge of its offset parent, rounded to
 * whole pixels. A node with no offset parent is measured from the window
 * origin.
 * @param el - the node to measure
 * @param builders - the lookup of a handle by node id
 * @returns `left` and `top`; both `0` when the node has no recorded box
 */
export function offsetOf(el: Element, builders: HandleBuilders): { left: number; top: number } {
  const box = layoutBox(el);
  if (!box) return { left: 0, top: 0 };

  const parent = offsetParentOf(el, builders);
  const origin = parent ? layoutBox(parent) : null;
  // The box moved with the scroll its parent applied when it was recorded.
  return {
    left: Math.round(box[0] - (origin ? origin[0] + origin[4] : 0) + box[5]) || 0,
    top: Math.round(box[1] - (origin ? origin[1] + origin[4] : 0) + box[6]) || 0,
  };
}

/** A rectangle with its position, its size and its four edges. `toJSON()` returns the plain values. */
export interface DOMRectReadOnly {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly top: number;
  readonly left: number;
  readonly right: number;
  readonly bottom: number;
  toJSON(): object;
}

/** A rectangle whose position and size can be assigned. */
export interface DOMRect extends DOMRectReadOnly {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** The members of a rect that hold values. */
type RectParts = Omit<DOMRectReadOnly, "toJSON">;

/**
 * Builds a rect from a node's box. Values stay unrounded.
 * @param layout - the box from {@link NodeBox}, or `null` for no record
 * @returns the rect; all zeros for `null`
 */
export function rectOf(layout: NodeBox | null): DOMRect {
  const [x, y, width, height] = layout ?? [0, 0, 0, 0, 0];
  const values: RectParts = {
    x,
    y,
    width,
    height,
    top: y,
    left: x,
    right: x + width,
    bottom: y + height,
  };
  return { ...values, toJSON: () => ({ ...values }) };
}

/**
 * Reads a `tabindex` attribute value as the element's tab index. A value that
 * is missing, not a string or number, has no leading integer, or falls outside
 * the 32-bit signed range yields the default.
 * @param value - the stored `tabindex` attribute, or `null` if unset
 * @param kind - the element kind; `"button"` and `"input"` default to `0`, any other kind to
 *   `-1`
 * @returns the parsed tab index, or the default
 */
export function tabIndexOf(value: string | number | boolean | null, kind: string): number {
  const fallback = kind === "button" || kind === "input" ? 0 : -1;
  if (typeof value !== "string" && typeof value !== "number") return fallback;

  const match = /^[\t\n\f\r ]*([+-]?\d+)/.exec(String(value));
  if (!match) return fallback;

  const n = Number(match[1]) || 0;
  return n >= -2147483648 && n <= 2147483647 ? n : fallback;
}

/**
 * Reads the box of the receiver of a layout member.
 * @param self - `this` of the member
 * @param rule - the receiver rule of the member
 * @returns the box, or `null`
 */
function boxOf(self: unknown, rule: Receiver): NodeBox | null {
  return layoutBox(receiverNode<Element>(self, rule));
}

/**
 * Builds the layout members of `Element`, read from the box of the last drawn frame.
 * @returns the members
 */
export function layoutMembers(): LayoutMembers {
  return {
    getBoundingClientRect() {
      return rectOf(boxOf(this, ELEMENT_RULE));
    },
    get clientWidth() {
      const box = boxOf(this, ELEMENT_RULE);
      return box ? Math.max(0, Math.round(box[2] - 2 * box[4] - box[7])) : 0;
    },
    get clientHeight() {
      const box = boxOf(this, ELEMENT_RULE);
      return box ? Math.max(0, Math.round(box[3] - 2 * box[4] - box[8])) : 0;
    },
    get clientLeft() {
      return Math.round(boxOf(this, ELEMENT_RULE)?.[4] ?? 0);
    },
    get clientTop() {
      return Math.round(boxOf(this, ELEMENT_RULE)?.[4] ?? 0);
    },
  };
}

/**
 * Builds the offset members of `HTMLElement`, read from the box of the last drawn frame.
 * @param builders - the lookup of a handle by node id
 * @returns the members
 */
export function offsetMembers(builders: HandleBuilders): OffsetMembers {
  return {
    get offsetWidth() {
      return Math.round(boxOf(this, HTML_ELEMENT_RULE)?.[2] ?? 0);
    },
    get offsetHeight() {
      return Math.round(boxOf(this, HTML_ELEMENT_RULE)?.[3] ?? 0);
    },
    get offsetLeft() {
      return offsetOf(receiverNode<Element>(this, HTML_ELEMENT_RULE), builders).left;
    },
    get offsetTop() {
      return offsetOf(receiverNode<Element>(this, HTML_ELEMENT_RULE), builders).top;
    },
    get offsetParent() {
      return offsetParentOf(receiverNode<Element>(this, HTML_ELEMENT_RULE), builders);
    },
  };
}
