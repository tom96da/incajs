// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import * as core from "../rendererCore.mts";
import { createNodeOps } from "./nodeOps.mts";
import { createPatchProp } from "./patchProp.mts";
import type { IncaElement } from "../rendererCore.mts";

describe("patchProp on a removed element", () => {
  it("adds a listener slot without reaching the host", () => {
    const released = new Set<number>();
    globalThis.__inca_native__ = {
      createNode: vi.fn<() => number>(() => 2),
      removeChild: vi.fn<() => void>(),
      holdNode: vi.fn<() => void>(),
      releaseNode: vi.fn<() => void>(),
      releaseListeners: vi.fn<(id: number) => number[]>((id) => {
        released.add(id);
        return [];
      }),
      addEventListener: vi.fn<(id: number) => number>((id) => {
        if (released.has(id)) throw new TypeError(`node ${id} is not live`);
        return 0;
      }),
      removeEventListener: vi.fn<() => boolean>(() => true),
    } as never;
    const ops = createNodeOps(core);
    const el = ops.createElement("view") as IncaElement;
    ops.remove(el);

    const patch = createPatchProp(core);

    expect(() => patch(el, "onClick", null, () => {}, undefined, null)).not.toThrow();
  });
});
