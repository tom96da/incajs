// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { offsetOf, rectOf, tabIndexOf } from "./elementGeometry.mts";
import { createIncaElement } from "./createElement.mts";
import type { IncaCore } from "../rendererCore.mts";

type Box = readonly [number, number, number, number, number, number, number];

function makeCore(boxes: Record<number, Box>, scrollOf = vi.fn<() => null>(() => null)): IncaCore {
  return {
    layoutOf: vi.fn<(nodeId: number) => Box | null>((nodeId) => boxes[nodeId] ?? null),
    scrollOf,
    createEventTarget: vi.fn<() => object>(() => ({})),
  } as unknown as IncaCore;
}

describe("offsetOf", () => {
  it("measures from the parent's padding edge inside its border", () => {
    const core = makeCore({ 1: [100, 50, 200, 100, 4, 0, 0], 2: [110, 60, 20, 20, 0, 0, 0] });
    const parent = createIncaElement(core, 1, "element", "div");
    const child = createIncaElement(core, 2, "element", "div");
    child.parent = parent;

    expect(offsetOf(core, child)).toEqual({ left: 6, top: 6 });
  });

  it("adds the scroll recorded with the box and ignores a later write", () => {
    const scrollOf = vi.fn<() => readonly [number, number, number, number]>(() => [90, 90, 0, 300]);
    const core = makeCore(
      { 1: [100, 50, 200, 100, 4, 0, 0], 2: [95, 20, 20, 20, 0, 15, 40] },
      scrollOf as never,
    );
    const parent = createIncaElement(core, 1, "element", "div");
    const child = createIncaElement(core, 2, "element", "div");
    child.parent = parent;

    expect(offsetOf(core, child)).toEqual({ left: 6, top: 6 });
    expect(scrollOf).not.toHaveBeenCalled();
  });
});

describe("rectOf", () => {
  it("derives the edges from the box and keeps fractional values", () => {
    const rect = rectOf([10.5, 20.25, 30.75, 40, 2, 0, 0]);
    expect([rect.x, rect.y, rect.width, rect.height]).toEqual([10.5, 20.25, 30.75, 40]);
    expect([rect.left, rect.top, rect.right, rect.bottom]).toEqual([10.5, 20.25, 41.25, 60.25]);
  });

  it("returns the plain values from toJSON", () => {
    const rect = rectOf([1, 2, 3, 4, 0, 0, 0]);
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
    const rect = rectOf(null);
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

describe("tabIndexOf", () => {
  it("parses integers and applies the default on invalid input", () => {
    const cases: [string | number | boolean | null, string, number][] = [
      ["0", "div", 0],
      ["5", "div", 5],
      ["  7", "div", 7],
      ["\t\n\f\r 9", "div", 9],
      ["+3", "div", 3],
      ["-4", "div", -4],
      ["-0", "div", 0],
      ["12abc", "div", 12],
      ["1.5", "div", 1],
      [1.5, "div", 1],
      ["2147483647", "div", 2147483647],
      ["-2147483648", "div", -2147483648],
      ["2147483648", "div", -1],
      ["-2147483649", "div", -1],
      [1e10, "div", -1],
      [NaN, "div", -1],
      ["", "div", -1],
      [" ", "div", -1],
      ["\u000b5", "div", -1],
      ["+", "div", -1],
      ["-", "div", -1],
      ["abc", "div", -1],
      [null, "div", -1],
      [true, "div", -1],
      [false, "div", -1],
      [null, "button", 0],
      [true, "button", 0],
      ["x", "button", 0],
      ["3", "button", 3],
    ];

    const mismatches: string[] = [];
    for (const [value, kind, expected] of cases) {
      const actual = tabIndexOf(value, kind);
      if (!Object.is(actual, expected)) {
        mismatches.push(`${JSON.stringify(value)} (${kind}): expected ${expected}, got ${actual}`);
      }
    }
    expect(mismatches).toEqual([]);
  });
});
