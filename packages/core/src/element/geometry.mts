// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { layoutOf } from "../tree.mts";
import { isDestroyed } from "./lifecycle.mts";
import type { IncaElement } from "./types.mts";

/** `[x, y, width, height, border, scrollX, scrollY]` of a node's last drawn frame. */
export type LayoutBox = readonly [number, number, number, number, number, number, number];

/**
 * Reads the box a node was last drawn at. A destroyed node, or one with no
 * recorded box, yields `null`.
 * @param el - the node to read
 * @returns the box, or `null`
 */
export function layoutBox(el: IncaElement): LayoutBox | null {
  return isDestroyed(el) ? null : layoutOf(el.__node_id);
}

/**
 * The parent a node's offsets are measured from. It is the parent when both
 * the node and the parent have a recorded box.
 * @param el - the node to query
 * @returns the parent, or `null`
 */
export function offsetParentOf(el: IncaElement): IncaElement | null {
  const parent = el.parent;
  if (!layoutBox(el) || !parent || !layoutBox(parent)) return null;
  return parent;
}

/**
 * Where a node sits inside the padding edge of its offset parent, rounded to
 * whole pixels. A node with no offset parent is measured from the window
 * origin.
 * @param el - the node to measure
 * @returns `left` and `top`; both `0` when the node has no recorded box
 */
export function offsetOf(el: IncaElement): { left: number; top: number } {
  const box = layoutBox(el);
  if (!box) return { left: 0, top: 0 };

  const parent = offsetParentOf(el);
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
 * @param layout - the box from {@link LayoutBox}, or `null` for no record
 * @returns the rect; all zeros for `null`
 */
export function rectOf(layout: LayoutBox | null): RectLike {
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
