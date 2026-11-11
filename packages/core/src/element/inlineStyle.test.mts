// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import { createIncaElement } from "./handle.mts";
import type { CSSStyleDeclaration } from "./types.mts";

/** The first block `styleOf` gives. */
const block = { cssText: "keep" } as CSSStyleDeclaration;

/** Installs a fake host whose `styleOf` gives `block` first and a new block on each later call. */
function setup() {
  const host = {
    styleOf: vi
      .fn<(nodeId: number) => CSSStyleDeclaration>(() => ({ cssText: "" }) as CSSStyleDeclaration)
      .mockReturnValueOnce(block),
    setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
    removeAttribute: vi.fn<(nodeId: number, key: string) => void>(),
    cloneNode: vi.fn<(nodeId: number, deep: boolean) => [number, string][]>(() => [[2, "div"]]),
    holdNode: vi.fn<(nodeId: number) => void>(),
  };
  globalThis.__inca_native__ = nativeStub(host as never);
  block.cssText = "keep";
  return { host, el: createIncaElement(1, "element", "div") };
}

describe("style", () => {
  it("reads the block the host gives for the node", () => {
    const { el } = setup();

    expect(el.style).toBe(block);
  });

  it("asks the host for the block of its own node", () => {
    const { host, el } = setup();

    void el.style;

    expect(host.styleOf).toHaveBeenCalledWith(1);
  });

  it("asks the host for the block once", () => {
    const { host, el } = setup();

    void el.style;
    void el.style;

    expect(host.styleOf).toHaveBeenCalledTimes(1);
  });

  it("reads the same object after setAttribute", () => {
    const { el } = setup();
    const before = el.style;

    el.setAttribute("style", "width: 1px");

    expect(el.style).toBe(before);
  });

  it("reads the same object after removeAttribute", () => {
    const { el } = setup();
    const before = el.style;

    el.removeAttribute("style");

    expect(el.style).toBe(before);
  });

  it("gives a clone a block that differs from the original", () => {
    const { el } = setup();

    const clone = el.cloneNode();

    expect(clone.style).not.toBe(el.style);
  });

  it("asks the host for the block of the clone's node", () => {
    const { host, el } = setup();

    void el.cloneNode().style;

    expect(host.styleOf).toHaveBeenLastCalledWith(2);
  });

  it("writes an assigned string to cssText", () => {
    const { el } = setup();

    el.style = "width: 1px";

    expect(block.cssText).toBe("width: 1px");
  });

  it("gives a comment stand-in no style", () => {
    setup();

    expect("style" in createIncaElement(3, "comment")).toBe(false);
  });

  it("writes an assigned null as an empty cssText", () => {
    const { el } = setup();

    el.style = null;

    expect(block.cssText).toBe("");
  });

  it("throws a TypeError for an assigned symbol", () => {
    const { el } = setup();

    expect(() => (el.style = Symbol("s") as unknown as string)).toThrow(TypeError);
  });
});
