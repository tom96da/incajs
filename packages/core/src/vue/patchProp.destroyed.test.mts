// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { createNodeOps } from "./nodeOps.mts";
import { createPatchProp } from "./patchProp.mts";
import type { IncaElement } from "../element/types.mts";
import type { IncaCore } from "../rendererCore.mts";
import type { IncaEventTarget } from "../types.mts";

describe("patchProp on a destroyed element", () => {
  it("adds a listener slot without reaching the host", () => {
    const freed = new Set<number>();
    const core = {
      createNode: vi.fn<() => number>(() => 2),
      destroyNode: vi.fn<(id: number) => void>((id) => void freed.add(id)),
      createEventTarget: vi.fn<() => IncaEventTarget>(() => ({
        addEventListener: vi.fn<IncaEventTarget["addEventListener"]>(),
        removeEventListener: vi.fn<IncaEventTarget["removeEventListener"]>(),
        dispatchEvent: vi.fn<IncaEventTarget["dispatchEvent"]>(() => true),
      })),
      addEventListener: vi.fn<(id: number) => void>((id) => {
        if (freed.has(id)) throw new TypeError(`node ${id} is not live`);
      }),
      removeEventListener: vi.fn<(id: number) => void>(),
      latestEventId: vi.fn<() => number>(() => 0),
    } as unknown as IncaCore;
    const ops = createNodeOps(core);
    const el = ops.createElement("view") as IncaElement;
    ops.remove(el);

    const patch = createPatchProp(core);

    expect(() => patch(el, "onClick", null, () => {}, undefined, null)).not.toThrow();
  });
});
