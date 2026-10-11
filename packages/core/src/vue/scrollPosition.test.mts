// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { createIncaElement } from "./createElement.mts";
import { markDestroyed } from "./destroyedNodes.mts";
import { scrollPosition } from "./scrollPosition.mts";
import type { IncaCore } from "../rendererCore.mts";
import type { ScrollOptions } from "./scrollPosition.mts";

function makeCore(scroll: readonly [number, number, number, number] | null = null): IncaCore {
  return {
    scrollOf: vi.fn<() => readonly [number, number, number, number] | null>(() => scroll),
    scrollNode: vi.fn<(nodeId: number, left: number | null, top: number | null) => void>(),
    createEventTarget: vi.fn<() => object>(() => ({})),
    getAttribute: vi.fn<() => null>(() => null),
    layoutOf: vi.fn<() => null>(() => null),
  } as unknown as IncaCore;
}

describe("scrollPosition", () => {
  it("sets both axes from the number form", () => {
    const core = makeCore();
    scrollPosition(core, createIncaElement(core, 7, "element", "div"), [30, 40], false);
    expect(core.scrollNode).toHaveBeenCalledWith(7, 30, 40);
  });

  it("sets both axes from the options form and ignores behavior", () => {
    const core = makeCore();
    const el = createIncaElement(core, 7, "element", "div");
    scrollPosition(core, el, [{ left: 5, top: 9, behavior: "smooth" }], false);
    expect(core.scrollNode).toHaveBeenCalledWith(7, 5, 9);
  });

  it("passes null for an axis the call leaves out", () => {
    const core = makeCore();
    const el = createIncaElement(core, 7, "element", "div");
    scrollPosition(core, el, [undefined, 12], false);
    expect(core.scrollNode).toHaveBeenLastCalledWith(7, null, 12);
    scrollPosition(core, el, [{ top: 3 }], false);
    expect(core.scrollNode).toHaveBeenLastCalledWith(7, null, 3);
  });

  it("adds to the current offset for a relative call", () => {
    const core = makeCore([10, 20, 100, 200]);
    const el = createIncaElement(core, 7, "element", "div");
    scrollPosition(core, el, [5, -4], true);
    expect(core.scrollNode).toHaveBeenLastCalledWith(7, 15, 16);
  });

  it("turns non-finite values into 0", () => {
    const core = makeCore();
    const el = createIncaElement(core, 7, "element", "div");
    scrollPosition(core, el, [NaN, Infinity], false);
    expect(core.scrollNode).toHaveBeenLastCalledWith(7, 0, 0);
  });

  it("starts a relative call from 0 when there is no record", () => {
    const core = makeCore(null);
    scrollPosition(core, createIncaElement(core, 7, "element", "div"), [4], true);
    expect(core.scrollNode).toHaveBeenLastCalledWith(7, 4, null);
  });

  it("ignores a call on a removed element", () => {
    const core = makeCore([1, 2, 3, 4]);
    const el = createIncaElement(core, 7, "element", "div");
    markDestroyed(el);
    scrollPosition(core, el, [1, 1], true);
    expect(core.scrollNode).not.toHaveBeenCalled();
  });
});

describe("scroll members on the element handle", () => {
  it("read zeros with no record", () => {
    const el = createIncaElement(makeCore(null), 7, "element", "div");
    expect([el.scrollLeft, el.scrollTop, el.scrollWidth, el.scrollHeight]).toEqual([0, 0, 0, 0]);
  });

  it("report scrollWidth and scrollHeight as client size plus the maximum offset", () => {
    const el = createIncaElement(makeCore([0, 0, 30, 60]), 7, "element", "div");
    expect([el.scrollWidth, el.scrollHeight]).toEqual([30, 60]);
  });

  it("read zeros once removed", () => {
    const core = makeCore([5, 6, 7, 8]);
    const el = createIncaElement(core, 7, "element", "div");
    markDestroyed(el);
    expect([el.scrollTop, el.scrollWidth]).toEqual([0, 0]);
  });
});

describe("scroll setters and methods on the element handle", () => {
  it("writes scrollLeft on the horizontal axis only", () => {
    const core = makeCore();
    const el = createIncaElement(core, 7, "element", "div");
    el.scrollLeft = 30;
    expect(core.scrollNode).toHaveBeenLastCalledWith(7, 30, null);
  });

  it("writes scrollTop on the vertical axis only", () => {
    const core = makeCore();
    const el = createIncaElement(core, 7, "element", "div");
    el.scrollTop = 40;
    expect(core.scrollNode).toHaveBeenLastCalledWith(7, null, 40);
  });

  it("reads scrollLeft and scrollTop from the record", () => {
    const el = createIncaElement(makeCore([10, 20, 100, 200]), 7, "element", "div");
    expect([el.scrollLeft, el.scrollTop]).toEqual([10, 20]);
  });

  it("wires scrollTo and scrollBy to the number form", () => {
    const core = makeCore([5, 6, 100, 200]);
    const el = createIncaElement(core, 7, "element", "div");
    el.scrollTo(10, 20);
    expect(core.scrollNode).toHaveBeenLastCalledWith(7, 10, 20);
    el.scrollBy(1, 2);
    expect(core.scrollNode).toHaveBeenLastCalledWith(7, 6, 8);
  });

  it("does not throw for scrollTo(null)", () => {
    const core = makeCore();
    const el = createIncaElement(core, 7, "element", "div");
    expect(() => el.scrollTo(null as unknown as ScrollOptions)).not.toThrow();
    expect(core.scrollNode).toHaveBeenLastCalledWith(7, null, null);
  });
});
