// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  appendChild,
  createNode,
  destroyNode,
  insertBefore,
  removeChild,
  rootNodeId,
  setAttribute,
  setStyle,
} from "./tree.mts";

const native = {
  rootNodeId: vi.fn<() => number>(() => 42),
  createNode: vi.fn<(tag: string) => number>((_tag: string) => 1),
  appendChild: vi.fn<(parentId: number, childId: number) => void>(),
  insertBefore: vi.fn<(parentId: number, childId: number, anchorId: number | null) => void>(),
  removeChild: vi.fn<(parentId: number, childId: number) => void>(),
  setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  setStyle: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  addEventListener: vi.fn<(nodeId: number, event: string, callbackId: number) => void>(),
  removeEventListener: vi.fn<(nodeId: number, event: string, callbackId: number) => boolean>(
    () => true,
  ),
  destroyNode: vi.fn<(nodeId: number) => number[]>(() => []),
};

beforeEach(() => {
  vi.clearAllMocks();
  globalThis.__gpjsui_native__ = native;
});

describe("wrapper functions forward to __gpjsui_native__", () => {
  it("rootNodeId", () => {
    expect(rootNodeId()).toBe(42);
    expect(native.rootNodeId).toHaveBeenCalledWith();
  });

  it("createNode", () => {
    expect(createNode("div")).toBe(1);
    expect(native.createNode).toHaveBeenCalledWith("div");
  });

  it("appendChild", () => {
    appendChild(1, 2);
    expect(native.appendChild).toHaveBeenCalledWith(1, 2);
  });

  it("insertBefore", () => {
    insertBefore(1, 2, 3);
    expect(native.insertBefore).toHaveBeenCalledWith(1, 2, 3);
  });

  it("insertBefore with a null anchor", () => {
    insertBefore(1, 2, null);
    expect(native.insertBefore).toHaveBeenCalledWith(1, 2, null);
  });

  it("removeChild", () => {
    removeChild(1, 2);
    expect(native.removeChild).toHaveBeenCalledWith(1, 2);
  });

  it("setAttribute", () => {
    setAttribute(1, "label", "hello");
    expect(native.setAttribute).toHaveBeenCalledWith(1, "label", "hello");
  });

  it("destroyNode", () => {
    destroyNode(7);
    expect(native.destroyNode).toHaveBeenCalledWith(7);
  });

  it("setStyle", () => {
    setStyle(1, "display", "flex");
    expect(native.setStyle).toHaveBeenCalledWith(1, "display", "flex");
  });
});
