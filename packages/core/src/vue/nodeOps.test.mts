// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { reactive, ref } from "@vue/runtime-core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { unregisterElement } from "../element/lifecycle.mts";
import * as tree from "../element/shadowTree.mts";
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

describe("createNodeOps delegation", () => {
  it("creates nodes through the shadow tree", () => {
    const el = nodeOps.createElement("div");
    const text = nodeOps.createText("hi");
    const comment = nodeOps.createComment("v-if");

    expect([el.kind, text.kind, comment.kind]).toEqual(["element", "text", "comment"]);
    expect(core.createNode).toHaveBeenNthCalledWith(1, "div");
    expect(core.createNode).toHaveBeenNthCalledWith(2, "text");
    expect(core.setStyle).toHaveBeenCalledWith(comment.id, "display", "none");
  });

  it("updates, inserts and removes through the shadow tree", () => {
    const parent = nodeOps.createElement("div");
    const first = nodeOps.createElement("div");
    const second = nodeOps.createText("a");

    nodeOps.insert(first, parent);
    nodeOps.insert(second, parent, first);
    nodeOps.setText(second as ReturnType<typeof nodeOps.createText>, "b");

    expect(parent.children).toEqual([second, first]);
    expect(nodeOps.parentNode(first)).toBe(parent);
    expect(nodeOps.nextSibling(second)).toBe(first);
    expect(core.setAttribute).toHaveBeenLastCalledWith(second.id, "value", "b");

    nodeOps.setElementText(parent, "");
    expect(parent.children).toEqual([]);

    nodeOps.remove(first);
    expect(core.destroyNode).toHaveBeenLastCalledWith(first.id);
  });

  it("reads parentNode and nextSibling from the shadow tree", () => {
    const node = tree.createElementNode(core, "div");

    expect([nodeOps.parentNode(node), nodeOps.nextSibling(node)]).toEqual([null, null]);
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
