// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { activeElement } from "./activeElement.mts";
import { createIncaElement } from "./createElement.mts";
import { elements } from "./elements.mts";
import type { IncaCore } from "../rendererCore.mts";

const focused = vi.fn<() => number | null>(() => null);

const core = {
  createEventTarget: vi.fn<() => object>(() => ({})),
} as unknown as IncaCore;

beforeEach(() => {
  elements.clear();
  focused.mockReset();
  focused.mockReturnValue(null);
  globalThis.__inca_native__ = { focusedNode: focused } as never;
});

describe("activeElement", () => {
  it("returns the registered element of the focused node", () => {
    const el = createIncaElement(core, 21, "element", "div");
    focused.mockReturnValue(21);

    expect(activeElement()).toBe(el);
  });

  it("returns null when no node has focus", () => {
    createIncaElement(core, 22, "element", "div");

    expect(activeElement()).toBeNull();
  });

  it("returns null for a focused id with no registered element", () => {
    focused.mockReturnValue(999);

    expect(activeElement()).toBeNull();
  });
});
