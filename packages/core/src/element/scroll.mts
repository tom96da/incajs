// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { scrollIntoViewNode, scrollNode, scrollOf } from "../ffi/index.mts";
import { isRemoved, ELEMENT_RULE, receiverNode } from "./lifecycle.mts";
import type { ScrollAlignment } from "../types.mts";
import type { IncaElement } from "./types.mts";

/** The options form of `scrollTo()` and `scrollBy()`. `behavior` is accepted and ignored. */
export interface ScrollOptions {
  /** The horizontal offset. Omit to keep the current one. */
  left?: number;
  /** The vertical offset. Omit to keep the current one. */
  top?: number;
  /** `behavior` is ignored; every scroll applies at once. */
  behavior?: "auto" | "instant" | "smooth";
}

/** Either `(left, top)` or an options object. */
export type ScrollArgs = [left?: number, top?: number] | [options?: ScrollOptions];

/**
 * Moves a node's scroll offset. An omitted axis keeps its offset, and a
 * non-finite value counts as `0`. A removed node ignores the call.
 * @param el - the node to scroll
 * @param args - `(left, top)` or `{ left, top }`
 * @param relative - `true` adds the values to the current offset
 */
export function scrollPosition(el: IncaElement, args: ScrollArgs, relative: boolean): void {
  if (isRemoved(el)) return;

  const [first, second] = args;
  const [x, y] = typeof first === "object" ? [first?.left, first?.top] : [first, second];
  const [left, top] = relative ? (scrollOf(el.__node_id) ?? [0, 0]) : [0, 0];
  const target = (value: number | undefined, from: number): number | null => {
    if (value === undefined) return null;
    const amount = Number(value);
    return (relative ? from : 0) + (Number.isFinite(amount) ? amount : 0);
  };

  scrollNode(el.__node_id, target(x, left), target(y, top));
}

/** The options form of `scrollIntoView()`. `behavior` is accepted and ignored. */
export interface ScrollIntoViewOptions {
  /** The vertical alignment. Defaults to `"start"`. */
  block?: ScrollAlignment;
  /** The horizontal alignment. Defaults to `"nearest"`. */
  inline?: ScrollAlignment;
  /** `behavior` is ignored; every scroll applies at once. */
  behavior?: "auto" | "instant" | "smooth";
}

/**
 * Turns `scrollIntoView()`'s argument into the alignment pair. `true` or no
 * argument aligns to the start, `false` to the end, and an options object
 * takes each axis from its own key. An unknown string falls back to that
 * axis's default.
 * @param arg - the boolean or options object the caller passed
 * @returns the `block` and `inline` alignments to send to the host
 */
export function scrollIntoViewArgs(arg?: boolean | ScrollIntoViewOptions): {
  block: ScrollAlignment;
  inline: ScrollAlignment;
} {
  const alignments: readonly string[] = ["start", "center", "end", "nearest"];
  if (typeof arg === "object" && arg !== null) {
    return {
      block: alignments.includes(arg.block ?? "") ? arg.block! : "start",
      inline: alignments.includes(arg.inline ?? "") ? arg.inline! : "nearest",
    };
  }
  return { block: arg === false ? "end" : "start", inline: "nearest" };
}

/** The members {@link scrollMembers} builds. */
export type ScrollMembers = Pick<
  IncaElement,
  | "scrollLeft"
  | "scrollTop"
  | "scrollWidth"
  | "scrollHeight"
  | "scrollTo"
  | "scrollBy"
  | "scrollIntoView"
>;

/**
 * Builds the scroll members of `Element`. A removed element reads zeros and ignores the calls.
 * @returns the members
 */
export function scrollMembers(): ScrollMembers {
  const elementOf = (self: unknown): IncaElement => receiverNode<IncaElement>(self, ELEMENT_RULE);
  const scroll = (element: IncaElement) =>
    isRemoved(element) ? null : scrollOf(element.__node_id);
  return {
    get scrollLeft() {
      return scroll(elementOf(this))?.[0] ?? 0;
    },
    set scrollLeft(value: number) {
      scrollPosition(elementOf(this), [value], false);
    },
    get scrollTop() {
      return scroll(elementOf(this))?.[1] ?? 0;
    },
    set scrollTop(value: number) {
      scrollPosition(elementOf(this), [undefined, value], false);
    },
    get scrollWidth() {
      const element = elementOf(this);
      return element.clientWidth + Math.round(scroll(element)?.[2] ?? 0);
    },
    get scrollHeight() {
      const element = elementOf(this);
      return element.clientHeight + Math.round(scroll(element)?.[3] ?? 0);
    },
    scrollTo(...args: ScrollArgs) {
      scrollPosition(elementOf(this), args, false);
    },
    scrollBy(...args: ScrollArgs) {
      scrollPosition(elementOf(this), args, true);
    },
    scrollIntoView(...rest: [arg?: boolean | ScrollIntoViewOptions]) {
      const element = elementOf(this);
      if (isRemoved(element)) return;
      const { block, inline } = scrollIntoViewArgs(rest[0]);
      scrollIntoViewNode(element.__node_id, block, inline);
    },
  };
}
