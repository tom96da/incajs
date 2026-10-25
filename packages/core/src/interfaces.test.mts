// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it } from "vitest";

import { Document, ShadowRoot } from "./interfaces.mts";

const interfaces = { Document, ShadowRoot };

describe("the interface globals", () => {
  it.each(["Document", "ShadowRoot"] as const)("throws when %s is constructed", (name) => {
    expect(() => Reflect.construct(interfaces[name], [])).toThrow(TypeError);
  });

  it.each(["Document", "ShadowRoot"] as const)("installs %s as a non-enumerable global", (name) => {
    expect(Object.getOwnPropertyDescriptor(globalThis, name)).toEqual({
      value: interfaces[name],
      writable: true,
      configurable: true,
      enumerable: false,
    });
  });
});
