// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it } from "vitest";

import { scrollIntoViewArgs } from "./scrollIntoViewArgs.mts";

describe("scrollIntoViewArgs", () => {
  it("aligns to the start when no argument or true is given", () => {
    expect(scrollIntoViewArgs()).toEqual({ block: "start", inline: "nearest" });
    expect(scrollIntoViewArgs(true)).toEqual({ block: "start", inline: "nearest" });
  });

  it("aligns to the end when false is given", () => {
    expect(scrollIntoViewArgs(false)).toEqual({ block: "end", inline: "nearest" });
  });

  it("takes each axis from the options object", () => {
    expect(scrollIntoViewArgs({ block: "center", inline: "end" })).toEqual({
      block: "center",
      inline: "end",
    });
  });

  it("defaults a missing axis to its own default", () => {
    expect(scrollIntoViewArgs({ inline: "center" })).toEqual({ block: "start", inline: "center" });
    expect(scrollIntoViewArgs({ block: "end" })).toEqual({ block: "end", inline: "nearest" });
    expect(scrollIntoViewArgs({})).toEqual({ block: "start", inline: "nearest" });
  });

  it("falls back to the axis default for an unknown string", () => {
    const unknown = { block: "bottom", inline: "left" } as unknown as Parameters<
      typeof scrollIntoViewArgs
    >[0];
    expect(scrollIntoViewArgs(unknown)).toEqual({ block: "start", inline: "nearest" });
  });

  it("ignores behavior", () => {
    expect(scrollIntoViewArgs({ block: "end", behavior: "smooth" })).toEqual({
      block: "end",
      inline: "nearest",
    });
  });
});
