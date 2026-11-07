// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { reactive, ref } from "@vue/runtime-core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as core from "../rendererCore.mts";
import { createNodeOps } from "./nodeOps.mts";

let nextId = 1;

/** The host bindings the element handles call. */
const native = {
  createNode: vi.fn<(tag: string) => number>(() => nextId++),
  appendChild: vi.fn<(parentId: number, childId: number) => number>(() => 0),
  insertBefore: vi.fn<(parentId: number, childId: number, refId: number | null) => number>(() => 0),
  removeChild: vi.fn<(parentId: number, childId: number) => void>(),
  setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  setStyle: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
};

const nodeOps = createNodeOps(core);

beforeEach(() => {
  vi.clearAllMocks();
  globalThis.__inca_native__ = native as never;
});

describe("createNodeOps comment", () => {
  it("keeps the text of a comment", () => {
    const comment = nodeOps.createComment("note");

    expect(native.setAttribute).toHaveBeenCalledWith(comment.__node_id, "value", "note");
  });
});

describe("createNodeOps text", () => {
  it("writes the text of a text node through setText", () => {
    const text = nodeOps.createText("a");

    nodeOps.setText(text, "b");

    expect(native.setAttribute).toHaveBeenLastCalledWith(text.__node_id, "value", "b");
  });
});

describe("createNodeOps scope id", () => {
  it("sets the scope id as an empty attribute", () => {
    const el = nodeOps.createElement("div");

    nodeOps.setScopeId!(el, "data-v-1a2b");

    expect(native.setAttribute).toHaveBeenCalledWith(el.__node_id, "data-v-1a2b", "");
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
