// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { registerElement, unregisterElement } from "./element/lifecycle.mts";
import "./targets.mts";
import { window } from "./window.mts";
import type { IncaElement } from "./element/types.mts";

beforeEach(() => {
  globalThis.__inca_native__ = { rootNodeId: vi.fn<() => number>(() => 77) } as never;
});

describe("__inca_targets__", () => {
  afterEach(() => {
    unregisterElement(5);
    unregisterElement(77);
  });

  it("gives window for the root, a registered node's handle, else null", () => {
    const handle = { __node_id: 5 } as unknown as IncaElement;
    const atRoot = { __node_id: 77 } as unknown as IncaElement;
    registerElement(handle);
    registerElement(atRoot);

    const [root, found, missing] = globalThis.__inca_targets__([77, 5, 6]);

    expect(root).toBe(window);
    expect(found).toBe(handle);
    expect(missing).toBeNull();
  });

  it("stays in place when a script assigns to it", () => {
    const hook = globalThis.__inca_targets__;

    expect(() => {
      globalThis.__inca_targets__ = () => [];
    }).toThrow(TypeError);
    expect(globalThis.__inca_targets__).toBe(hook);
  });

  it("is the latest copy's once core loads again", async () => {
    const hook = globalThis.__inca_targets__;
    vi.resetModules();

    await import("./targets.mts");

    expect(globalThis.__inca_targets__).not.toBe(hook);
  });
});
