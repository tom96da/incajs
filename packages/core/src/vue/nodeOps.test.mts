// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { reactive, ref } from "@vue/runtime-core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { unregisterElement } from "../element/lifecycle.mts";
import { createNodeOps } from "./nodeOps.mts";
import type { IncaCore } from "../rendererCore.mts";

let nextId = 1;

const core = {
  createNode: vi.fn<(tag: string) => number>(),
  appendChild: vi.fn<(parentId: number, childId: number) => void>(),
  insertBefore: vi.fn<(parentId: number, childId: number, anchorId: number | null) => void>(),
  removeChild: vi.fn<(parentId: number, childId: number) => void>(),
  destroyNode: vi.fn<(nodeId: number) => void>(),
  setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  setStyle: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  createEventTarget: vi.fn<IncaCore["createEventTarget"]>(() => ({
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => true,
  })),
} as unknown as IncaCore;

const nodeOps = createNodeOps(core);

beforeEach(() => {
  vi.clearAllMocks();
  for (let id = 1; id < nextId; id++) unregisterElement(id);
  nextId = 1;
  vi.mocked(core.createNode).mockImplementation(() => nextId++);
});

describe("createNodeOps text", () => {
  it("writes the text of a text node through setText", () => {
    const text = nodeOps.createText("a");

    nodeOps.setText(text, "b");

    expect(core.setAttribute).toHaveBeenLastCalledWith(text.__node_id, "value", "b");
  });
});

describe("createNodeOps reactivity", () => {
  it("keeps the same element inside reactive() and ref()", () => {
    const el = nodeOps.createElement("div");

    expect(reactive(el)).toBe(el);
    expect(ref(el).value).toBe(el);
  });

  it("keeps the same text and comment handles inside reactive()", () => {
    const text = nodeOps.createText("t");
    const comment = nodeOps.createComment("c");

    expect(reactive(text)).toBe(text);
    expect(reactive(comment)).toBe(comment);
  });
});
