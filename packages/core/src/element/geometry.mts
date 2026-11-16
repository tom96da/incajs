// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { layoutOf, linkedNode } from "../ffi/index.mts";
import { isRemoved } from "./lifecycle.mts";
import type { NodeBox } from "../ffi/index.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type { IncaElement } from "./types.mts";

/** The members {@link layoutMembers} builds. */
export type LayoutMembers = Pick<
  IncaElement,
  | "getBoundingClientRect"
  | "offsetWidth"
  | "offsetHeight"
  | "offsetLeft"
  | "offsetTop"
  | "offsetParent"
  | "clientWidth"
  | "clientHeight"
  | "clientLeft"
  | "clientTop"
>;

/**
 * Reads the box a node was last drawn at. A removed node, or one with no
 * recorded box, yields `null`.
 * @param el - the node to read
 * @returns the box, or `null`
 */
export function layoutBox(el: IncaElement): NodeBox | null {
  return isRemoved(el) ? null : layoutOf(el.__node_id);
}

/**
 * The parent a node's offsets are measured from. It is the parent element when both
 * the node and the parent have a recorded box.
 * @param el - the node to query
 * @param builders - the lookup of a handle by node id
 * @returns the parent, or `null`
 */
export function offsetParentOf(el: IncaElement, builders: HandleBuilders): IncaElement | null {
  if (!layoutBox(el)) return null;
  const id = linkedNode(el.__node_id, "parent", true);
  const parent = id === null ? undefined : (builders.node(id) as IncaElement | undefined);
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
export function offsetOf(el: IncaElement, builders: HandleBuilders): { left: number; top: number } {
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

/** The plain values of a {@link RectLike}, as `toJSON` returns them. */
export interface RectValues {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly top: number;
  readonly left: number;
  readonly right: number;
  readonly bottom: number;
}

/**
 * A rectangle with its position, its size and its four edges. `toJSON()` returns
 * the plain values.
 */
export interface RectLike extends RectValues {
  toJSON(): RectValues;
}

/**
 * Builds a rect from a node's box. Values stay unrounded.
 * @param layout - the box from {@link NodeBox}, or `null` for no record
 * @returns the rect; all zeros for `null`
 */
export function rectOf(layout: NodeBox | null): RectLike {
  const [x, y, width, height] = layout ?? [0, 0, 0, 0, 0];
  const values: RectValues = {
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
 * Builds the layout members of an element, read from the box of the last drawn frame.
 * @param element - the handle the members belong to
 * @param builders - the lookup of a handle by node id
 * @returns the members
 */
export function layoutMembers(element: IncaElement, builders: HandleBuilders): LayoutMembers {
  return {
    getBoundingClientRect: () => rectOf(layoutBox(element)),
    get offsetWidth() {
      return Math.round(layoutBox(element)?.[2] ?? 0);
    },
    get offsetHeight() {
      return Math.round(layoutBox(element)?.[3] ?? 0);
    },
    get offsetLeft() {
      return offsetOf(element, builders).left;
    },
    get offsetTop() {
      return offsetOf(element, builders).top;
    },
    get offsetParent() {
      return offsetParentOf(element, builders);
    },
    get clientWidth() {
      const box = layoutBox(element);
      return box ? Math.max(0, Math.round(box[2] - 2 * box[4] - box[7])) : 0;
    },
    get clientHeight() {
      const box = layoutBox(element);
      return box ? Math.max(0, Math.round(box[3] - 2 * box[4] - box[8])) : 0;
    },
    get clientLeft() {
      return Math.round(layoutBox(element)?.[4] ?? 0);
    },
    get clientTop() {
      return Math.round(layoutBox(element)?.[4] ?? 0);
    },
  };
}
