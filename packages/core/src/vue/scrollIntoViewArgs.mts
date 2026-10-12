// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { ScrollAlignment } from "../types.mts";

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
