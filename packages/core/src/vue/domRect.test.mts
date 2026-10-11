// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it } from "vitest";

import { domRect } from "./domRect.mts";

describe("domRect", () => {
  it("derives the edges from the box and keeps fractional values", () => {
    const rect = domRect([10.5, 20.25, 30.75, 40, 2, 0, 0]);
    expect([rect.x, rect.y, rect.width, rect.height]).toEqual([10.5, 20.25, 30.75, 40]);
    expect([rect.left, rect.top, rect.right, rect.bottom]).toEqual([10.5, 20.25, 41.25, 60.25]);
  });

  it("returns the plain values from toJSON", () => {
    const rect = domRect([1, 2, 3, 4, 0, 0, 0]);
    expect(rect.toJSON()).toEqual({
      x: 1,
      y: 2,
      width: 3,
      height: 4,
      top: 2,
      left: 1,
      right: 4,
      bottom: 6,
    });
  });

  it("gives all zeros for a null layout", () => {
    const rect = domRect(null);
    expect([
      rect.x,
      rect.y,
      rect.width,
      rect.height,
      rect.top,
      rect.left,
      rect.right,
      rect.bottom,
    ]).toEqual([0, 0, 0, 0, 0, 0, 0, 0]);
  });
});
