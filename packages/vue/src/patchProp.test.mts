// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { createPatchProp } from "./patchProp.mts";
import type { IncajsCore } from "./core.mts";
import type { GpjsuiElement } from "./nodeOps.mts";

const core: IncajsCore = {
  rootNodeId: vi.fn<() => number>(),
  createNode: vi.fn<(tag: string) => number>(),
  appendChild: vi.fn<(parentId: number, childId: number) => void>(),
  insertBefore: vi.fn<(parentId: number, childId: number, anchorId: number | null) => void>(),
  removeChild: vi.fn<(parentId: number, childId: number) => void>(),
  destroyNode: vi.fn<(nodeId: number) => void>(),
  setEventListener:
    vi.fn<(nodeId: number, event: string, listener: (...args: unknown[]) => void) => void>(),
  removeEventListener: vi.fn<(nodeId: number, event: string) => void>(),
  setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  setStyle: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
};

const patchProp = createPatchProp(core);

const el: GpjsuiElement = { id: 1, kind: "element", parent: null, children: [] };

beforeEach(() => {
  vi.clearAllMocks();
});

describe("style", () => {
  it("calls setStyle once per primitive-valued entry", () => {
    patchProp(el, "style", null, { background: "#000", gap: 8 }, undefined, null);

    expect(core.setStyle).toHaveBeenCalledWith(1, "background", "#000");
    expect(core.setStyle).toHaveBeenCalledWith(1, "gap", 8);
    expect(core.setStyle).toHaveBeenCalledTimes(2);
  });

  it("skips entries whose value isn't a primitive setStyle can take", () => {
    patchProp(el, "style", null, { border_width: [1, 2] }, undefined, null);

    expect(core.setStyle).not.toHaveBeenCalled();
  });

  it("does nothing for a null style value", () => {
    patchProp(el, "style", { background: "#000" }, null, undefined, null);

    expect(core.setStyle).not.toHaveBeenCalled();
  });
});

describe("on*", () => {
  it("registers a function listener under the lower-cased event name", () => {
    const listener = vi.fn<() => void>();
    patchProp(el, "onClick", null, listener, undefined, null);

    expect(core.setEventListener).toHaveBeenCalledWith(1, "click", listener);
  });

  it("collapses an array of handlers into one listener", () => {
    const first = vi.fn<() => void>();
    const second = vi.fn<() => void>();

    patchProp(el, "onClick", null, [first, second], undefined, null);

    expect(core.setEventListener).toHaveBeenCalledTimes(1);
    const registered = vi.mocked(core.setEventListener).mock.calls[0]![2];
    registered();
    expect(first).toHaveBeenCalledTimes(1);
    expect(second).toHaveBeenCalledTimes(1);
  });

  it("unbinds the event when the value is no longer a function", () => {
    patchProp(el, "onClick", vi.fn(), undefined, undefined, null);

    expect(core.setEventListener).not.toHaveBeenCalled();
    expect(core.removeEventListener).toHaveBeenCalledWith(1, "click");
  });
});

describe("everything else", () => {
  it.each([
    ["label", "hello"],
    ["count", 3],
    ["disabled", true],
  ])("forwards %s=%p to setAttribute", (key, value) => {
    patchProp(el, key, null, value, undefined, null);

    expect(core.setAttribute).toHaveBeenCalledWith(1, key, value);
  });

  it("skips a value setAttribute can't take", () => {
    patchProp(el, "data", null, { nested: true }, undefined, null);

    expect(core.setAttribute).not.toHaveBeenCalled();
  });
});
