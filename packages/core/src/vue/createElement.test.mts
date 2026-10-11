// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { reactive, ref } from "@vue/runtime-core";
import { describe, expect, it, vi } from "vitest";

import { createIncaElement } from "./createElement.mts";
import { markDestroyed } from "./destroyedNodes.mts";
import { elements } from "./elements.mts";
import type { IncaCore } from "../rendererCore.mts";

function makeCore(
  stored: string | number | boolean | null = null,
  boxes: Record<number, readonly [number, number, number, number, number]> = {},
): IncaCore {
  return {
    getAttribute: vi.fn<(nodeId: number, key: string) => string | number | boolean | null>(
      () => stored,
    ),
    layoutOf: vi.fn<(nodeId: number) => readonly [number, number, number, number, number] | null>(
      (nodeId) => boxes[nodeId] ?? null,
    ),
    setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
    focus: vi.fn<() => void>(),
    blur: vi.fn<() => void>(),
    click: vi.fn<() => void>(),
    createEventTarget: vi.fn<() => object>(() => ({})),
  } as unknown as IncaCore;
}

describe("createIncaElement tabIndex", () => {
  it("reads -1 for a div and 0 for a button when no tabindex is stored", () => {
    expect(createIncaElement(makeCore(), 1, "element", "div").tabIndex).toBe(-1);
    expect(createIncaElement(makeCore(), 2, "element", "button").tabIndex).toBe(0);
    expect(createIncaElement(makeCore(), 3, "comment").tabIndex).toBe(-1);
  });

  it("reads the stored tabindex attribute", () => {
    const core = makeCore("4");
    expect(createIncaElement(core, 1, "element", "div").tabIndex).toBe(4);
    expect(core.getAttribute).toHaveBeenCalledWith(1, "tabindex");
  });

  it("writes the value as a 32-bit integer", () => {
    const core = makeCore();
    const el = createIncaElement(core, 1, "element", "div");

    el.tabIndex = NaN;
    expect(core.setAttribute).toHaveBeenLastCalledWith(1, "tabindex", 0);

    el.tabIndex = 1e10;
    expect(core.setAttribute).toHaveBeenLastCalledWith(1, "tabindex", 1410065408);

    el.tabIndex = -1.5;
    expect(core.setAttribute).toHaveBeenLastCalledWith(1, "tabindex", -1);

    el.tabIndex = Infinity;
    expect(core.setAttribute).toHaveBeenLastCalledWith(1, "tabindex", 0);
  });
});

describe("createIncaElement layout", () => {
  it("reads the rect from the last drawn box", () => {
    const el = createIncaElement(makeCore(null, { 1: [10, 20, 30, 40, 2] }), 1, "element", "div");
    const rect = el.getBoundingClientRect();
    expect([
      rect.x,
      rect.y,
      rect.width,
      rect.height,
      rect.left,
      rect.top,
      rect.right,
      rect.bottom,
    ]).toEqual([10, 20, 30, 40, 10, 20, 40, 60]);
  });

  it("gives zeros and a null offsetParent when there is no record", () => {
    const el = createIncaElement(makeCore(), 1, "element", "div");
    const rect = el.getBoundingClientRect();
    expect([rect.x, rect.y, rect.width, rect.height, rect.right, rect.bottom]).toEqual([
      0, 0, 0, 0, 0, 0,
    ]);
    expect([el.offsetWidth, el.offsetHeight, el.offsetLeft, el.offsetTop]).toEqual([0, 0, 0, 0]);
    expect([el.clientWidth, el.clientHeight, el.clientLeft, el.clientTop]).toEqual([0, 0, 0, 0]);
    expect(el.offsetParent).toBeNull();
  });

  it("rounds the offset and client members", () => {
    const el = createIncaElement(
      makeCore(null, { 1: [10.4, 20.6, 30.5, 40.2, 1.6] }),
      1,
      "element",
      "div",
    );
    expect([el.offsetWidth, el.offsetHeight]).toEqual([31, 40]);
    expect([el.offsetLeft, el.offsetTop]).toEqual([10, 21]);
    expect([el.clientLeft, el.clientTop]).toEqual([2, 2]);
    expect([el.clientWidth, el.clientHeight]).toEqual([27, 37]);
  });

  it("measures offsets from inside the border of the offsetParent", () => {
    const core = makeCore(null, { 1: [100, 50, 200, 100, 4], 2: [110, 60, 20, 20, 0] });
    const parent = createIncaElement(core, 1, "element", "div");
    const child = createIncaElement(core, 2, "element", "div");
    child.parent = parent;

    expect([child.offsetLeft, child.offsetTop]).toEqual([6, 6]);
  });

  it("reads a position slightly above the parent as 0", () => {
    const core = makeCore(null, { 1: [100, 50, 200, 100, 0], 2: [99.7, 49.7, 20, 20, 0] });
    const parent = createIncaElement(core, 1, "element", "div");
    const child = createIncaElement(core, 2, "element", "div");
    child.parent = parent;

    expect([child.offsetLeft, child.offsetTop]).toEqual([0, 0]);
  });

  it("clamps the client size at zero", () => {
    const el = createIncaElement(makeCore(null, { 1: [0, 0, 2, 2, 5] }), 1, "element", "div");
    expect([el.clientWidth, el.clientHeight]).toEqual([0, 0]);
  });

  it("measures offsets from the offsetParent", () => {
    const core = makeCore(null, { 1: [100, 50, 200, 100, 0], 2: [110.4, 60.6, 20, 20, 0] });
    const parent = createIncaElement(core, 1, "element", "div");
    const child = createIncaElement(core, 2, "element", "div");
    child.parent = parent;

    expect(child.offsetParent).toBe(parent);
    expect([child.offsetLeft, child.offsetTop]).toEqual([10, 11]);
  });

  it("gives a null offsetParent when the parent has no record", () => {
    const core = makeCore(null, { 2: [110, 60, 20, 20, 0] });
    const child = createIncaElement(core, 2, "element", "div");
    child.parent = createIncaElement(core, 1, "element", "div");

    expect(child.offsetParent).toBeNull();
    expect([child.offsetLeft, child.offsetTop]).toEqual([110, 60]);
  });

  it("gives a null offsetParent to the root wrapper", () => {
    const el = createIncaElement(makeCore(null, { 1: [5.4, 6, 10, 10, 0] }), 1, "element", "div");
    expect(el.offsetParent).toBeNull();
    expect([el.offsetLeft, el.offsetTop]).toEqual([5, 6]);
  });

  it("reads zeros for a destroyed element", () => {
    const el = createIncaElement(makeCore(null, { 1: [10, 20, 30, 40, 2] }), 1, "element", "div");
    markDestroyed(el);
    expect([
      el.getBoundingClientRect().width,
      el.offsetWidth,
      el.clientWidth,
      el.offsetLeft,
    ]).toEqual([0, 0, 0, 0]);
    expect(el.offsetParent).toBeNull();
  });
});

describe("createIncaElement reactivity", () => {
  it("keeps the same object inside reactive() and ref()", () => {
    const el = createIncaElement(makeCore(), 1, "element", "div");

    expect(reactive(el)).toBe(el);
    expect(ref(el).value).toBe(el);
  });

  it("registers an element and leaves a comment stand-in out", () => {
    const el = createIncaElement(makeCore(), 7, "element", "div");
    createIncaElement(makeCore(), 8, "comment");

    expect([elements.get(7), elements.has(8)]).toEqual([el, false]);
  });
});
