// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it } from "vitest";

import { globalsOfDefine } from "../../src/vite/hmr.mts";

describe("globalsOfDefine", () => {
  it("reads a non-string value as its JSON form", () => {
    expect(globalsOfDefine({ __APP_CONFIG__: { a: 1 } })).toEqual({ __APP_CONFIG__: { a: 1 } });
  });

  it("reads a string value as JSON code", () => {
    expect(globalsOfDefine({ __APP_NAME__: '"inca"' })).toEqual({ __APP_NAME__: "inca" });
  });

  it("skips a key that is not a plain identifier", () => {
    expect(globalsOfDefine({ "process.env.NODE_ENV": '"development"' })).toEqual({});
  });

  it("skips code that is not a JSON literal", () => {
    expect(globalsOfDefine({ __APP_GLOBAL__: "window" })).toEqual({});
  });

  it("gives no globals without a define", () => {
    expect(globalsOfDefine(undefined)).toEqual({});
  });
});
