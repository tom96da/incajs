// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { ScrollAlignment } from "../types.mts";

/** The options form of `scrollTo()` and `scrollBy()`. `behavior` is accepted and ignored. */
export interface ScrollOptions {
  /** The horizontal offset. Omit to keep the current one. */
  left?: number;
  /** The vertical offset. Omit to keep the current one. */
  top?: number;
  /** `behavior` is ignored; every scroll applies at once. */
  behavior?: "auto" | "instant" | "smooth";
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
