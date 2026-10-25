// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it } from "vitest";

import { defaultConfig, defineConfig } from "./index.mts";

describe("defineConfig", () => {
  it("is an identity function at runtime", () => {
    const config = { productName: "Click Counter" };
    expect(defineConfig(config)).toBe(config);
  });
});

describe("defaultConfig", () => {
  it("defaults to dist, no source map, version 0.0.0 and relative stack paths", () => {
    expect(defaultConfig).toEqual({
      dev: { stackPaths: "relative" },
      outDir: "dist",
      sourcemap: false,
      version: "0.0.0",
    });
  });
});
