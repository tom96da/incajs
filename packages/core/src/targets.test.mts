// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { nativeStub } from "../tests/setup.mts";
import "./targets.mts";
import { document } from "./document.mts";
import { adoptTextNode } from "./element/handle.mts";
import { elementOf, registerNode, unregisterNode } from "./element/lifecycle.mts";
import { window } from "./window.mts";
import type { IncaElement } from "./element/types.mts";

/** The id `nativeStub` gives the document. */
const DOCUMENT = 1;
const ROOT = 77;

beforeEach(() => {
  globalThis.__inca_native__ = nativeStub({ rootNodeId: vi.fn<() => number>(() => ROOT) } as never);
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
    unregisterNode(5);
    unregisterNode(ROOT);
    unregisterNode(8);
  });

  it("resolves the id of the document node to the document", () => {
    const [doc] = globalThis.__inca_targets__([DOCUMENT]);

    expect(doc).toBe(document);
  });

  it("resolves the window tag to window", () => {
    const [win] = globalThis.__inca_targets__(["$window"]);

    expect(win).toBe(window);
  });

  it("returns null for a text node", () => {
    const text = adoptTextNode(8, "t");

    const [found] = globalThis.__inca_targets__([text.__node_id]);

    expect(found).toBeNull();
  });

  it("resolves registered element nodes to their handles", () => {
    const handle = { __node_id: 5, kind: "element" } as unknown as IncaElement;
    registerNode(handle);

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
