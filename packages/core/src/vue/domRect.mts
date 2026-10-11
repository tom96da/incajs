// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { LayoutBox } from "./layoutBox.mts";

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

/** The shape {@link IncaElement.getBoundingClientRect} returns. */
export interface RectLike extends RectValues {
  toJSON(): RectValues;
}

/**
 * Builds a rect from a node's box. Values stay unrounded.
 * @param layout - the box from {@link LayoutBox}, or `null` for no record
 * @returns the rect; all zeros for `null`
 */
export function domRect(layout: LayoutBox | null): RectLike {
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
