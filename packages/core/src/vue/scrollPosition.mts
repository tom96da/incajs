// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { isDestroyed } from "./destroyedNodes.mts";
import type { IncaCore } from "../rendererCore.mts";
import type { IncaElement } from "./nodeOps.mts";

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
 * non-finite value counts as `0`. A destroyed node ignores the call.
 * @param core - the incajs bindings to write through
 * @param el - the node to scroll
 * @param args - `(left, top)` or `{ left, top }`
 * @param relative - `true` adds the values to the current offset
 */
export function scrollPosition(
  core: IncaCore,
  el: IncaElement,
  args: ScrollArgs,
  relative: boolean,
): void {
  if (isDestroyed(el)) return;

  const [first, second] = args;
  const [x, y] = typeof first === "object" ? [first?.left, first?.top] : [first, second];
  const [left, top] = relative ? (core.scrollOf(el.id) ?? [0, 0]) : [0, 0];
  const target = (value: number | undefined, from: number): number | null => {
    if (value === undefined) return null;
    const amount = Number(value);
    return (relative ? from : 0) + (Number.isFinite(amount) ? amount : 0);
  };

  core.scrollNode(el.id, target(x, left), target(y, top));
}
