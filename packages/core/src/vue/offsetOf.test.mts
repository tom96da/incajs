// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { createIncaElement } from "./createElement.mts";
import { offsetOf } from "./offsetOf.mts";
import type { IncaCore } from "../rendererCore.mts";

type Box = readonly [number, number, number, number, number, number, number];

function makeCore(boxes: Record<number, Box>, scrollOf = vi.fn<() => null>(() => null)): IncaCore {
  return {
    layoutOf: vi.fn<(nodeId: number) => Box | null>((nodeId) => boxes[nodeId] ?? null),
    scrollOf,
    createEventTarget: vi.fn<() => object>(() => ({})),
  } as unknown as IncaCore;
}

describe("offsetOf", () => {
  it("measures from the parent's padding edge inside its border", () => {
    const core = makeCore({ 1: [100, 50, 200, 100, 4, 0, 0], 2: [110, 60, 20, 20, 0, 0, 0] });
    const parent = createIncaElement(core, 1, "element", "div");
    const child = createIncaElement(core, 2, "element", "div");
    child.parent = parent;

    expect(offsetOf(core, child)).toEqual({ left: 6, top: 6 });
  });

  it("adds the scroll recorded with the box and ignores a later write", () => {
    const scrollOf = vi.fn<() => readonly [number, number, number, number]>(() => [90, 90, 0, 300]);
    const core = makeCore(
      { 1: [100, 50, 200, 100, 4, 0, 0], 2: [95, 20, 20, 20, 0, 15, 40] },
      scrollOf as never,
    );
    const parent = createIncaElement(core, 1, "element", "div");
    const child = createIncaElement(core, 2, "element", "div");
    child.parent = parent;

    expect(offsetOf(core, child)).toEqual({ left: 6, top: 6 });
    expect(scrollOf).not.toHaveBeenCalled();
  });
});
