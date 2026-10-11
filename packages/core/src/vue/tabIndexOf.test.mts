// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it } from "vitest";

import { tabIndexOf } from "./tabIndexOf.mts";

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
