// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { createIncaElement } from "./createElement.mts";
import { markDestroyed } from "./destroyedNodes.mts";
import { scrollPosition } from "./scrollPosition.mts";
import type { IncaCore } from "../rendererCore.mts";

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
