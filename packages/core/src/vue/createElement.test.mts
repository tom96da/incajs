// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { createIncaElement } from "./createElement.mts";
import type { IncaCore } from "../rendererCore.mts";

function makeCore(stored: string | number | boolean | null = null): IncaCore {
  return {
    getAttribute: vi.fn<(nodeId: number, key: string) => string | number | boolean | null>(
      () => stored,
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
