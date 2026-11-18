// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { layoutOf, linkedNode } from "../ffi/index.mts";
import { HTML_ELEMENT_RULE, receiverNode } from "./lifecycle.mts";
import type { NodeBox } from "../ffi/index.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type { Element, HTMLElement } from "./types.mts";

/** The members {@link offsetMembers} builds. */
export type OffsetMembers = Pick<HTMLElement, "offsetParent">;

/**
 * Reads the box a node was last drawn at. A node that is not connected, or one with no recorded
 * box, yields `null`.
 * @param el - the node to read
 * @returns the box, or `null`
 */
function layoutBox(el: Element): NodeBox | null {
  return layoutOf(el.__node_id);
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
 * Builds `offsetParent` of `HTMLElement`. The host defines the other offset members.
 * @param builders - the lookup of a handle by node id
 * @returns the members
 */
export function offsetMembers(builders: HandleBuilders): OffsetMembers {
  return {
    get offsetParent() {
      return offsetParentOf(receiverNode<Element>(this, HTML_ELEMENT_RULE), builders);
    },
  };
}
