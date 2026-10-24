// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { document } from "./document.mts";
import "./targets.mts";
import { elementOf, registerElement, unregisterElement } from "./element/lifecycle.mts";
import { window } from "./window.mts";
import type { IncaElement } from "./element/types.mts";

const ROOT = 77;

beforeEach(() => {
  globalThis.__inca_native__ = { rootNodeId: vi.fn<() => number>(() => ROOT) } as never;
});

describe("globals", () => {
  it("leaves globalThis.document and globalThis.window undefined", () => {
    expect([
      (globalThis as { document?: unknown }).document,
      (globalThis as { window?: unknown }).window,
    ]).toEqual([undefined, undefined]);
  });
});

describe("__inca_targets__", () => {
  afterEach(() => {
    unregisterElement(5);
    unregisterElement(ROOT);
  });

  it("resolves the document and window tags to their objects", () => {
    const [doc, win] = globalThis.__inca_targets__(["$document", "$window"]);

    expect(doc).toBe(document);
    expect(win).toBe(window);
  });

  it("resolves registered element nodes to their handles", () => {
    const handle = { __node_id: 5 } as unknown as IncaElement;
    registerElement(handle);

    const [found] = globalThis.__inca_targets__([5]);

    expect(found).toBe(handle);
  });

  it("returns null for unknown nodes", () => {
    const [missing] = globalThis.__inca_targets__([6]);

    expect(missing).toBeNull();
  });

  it("creates the element of the host root on first use", () => {
    const [root] = globalThis.__inca_targets__([ROOT]) as [IncaElement];

    expect(root.__node_id).toBe(ROOT);
    expect(elementOf(ROOT)).toBe(root);
  });

  it("gives the live element of the host root again", () => {
    const [first] = globalThis.__inca_targets__([ROOT]);
    const [second] = globalThis.__inca_targets__([ROOT]);

    expect(second).toBe(first);
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
