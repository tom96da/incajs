// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { adoptComment, adoptFragment, adoptText } from "../tests/adopt.mts";
import "./targets.mts";
import { nativeStub } from "../tests/setup.mts";
import { document } from "./document.mts";
import { elementOf, registerNode, markFreed } from "./element/lifecycle.mts";
import { windowTargetId } from "./ffi/index.mts";
import { window } from "./window.mts";
import type { Element } from "./element/types.mts";

/** The id `nativeStub` gives the document. */
const DOCUMENT = 1;
const ROOT = 77;

beforeEach(() => {
  vi.stubGlobal(
    "__inca_native__",
    nativeStub({ rootNodeId: vi.fn<() => number>(() => ROOT) } as never),
  );
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
    markFreed([5]);
    markFreed([ROOT]);
    markFreed([8]);
    markFreed([6]);
  });

  it("resolves the id of the document node to the document", () => {
    const [doc] = globalThis.__inca_targets__([DOCUMENT]);

    expect(doc).toBe(document);
  });

  it("resolves the window target to window", () => {
    const [win] = globalThis.__inca_targets__([windowTargetId()]);

    expect(win).toBe(window);
  });

  it.each([
    ["a text node", () => adoptText(8, "t")],
    ["a comment", () => adoptComment(8)],
    ["a fragment", () => adoptFragment(8)],
  ])("resolves %s to its handle", (_kind, make) => {
    const handle = make();

    const [found] = globalThis.__inca_targets__([handle.__node_id]);

    expect(found).toBe(handle);
  });

  it("resolves registered element nodes to their handles", () => {
    const handle = { __node_id: 5, kind: "element" } as unknown as Element;
    registerNode(handle);

    const [found] = globalThis.__inca_targets__([5]);

    expect(found).toBe(handle);
  });

  it("builds the handle of a node that has none", () => {
    const [built] = globalThis.__inca_targets__([6]) as [Element];

    expect([built.__node_id, elementOf(6)]).toEqual([6, built]);
  });

  it("returns null for a node the host no longer has", () => {
    vi.stubGlobal("__inca_native__", { ...globalThis.__inca_native__, nodeHandle: () => null });

    const [missing] = globalThis.__inca_targets__([6]);

    expect(missing).toBeNull();
  });

  it("creates the element of the host root on first use", () => {
    const [root] = globalThis.__inca_targets__([ROOT]) as [Element];

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
