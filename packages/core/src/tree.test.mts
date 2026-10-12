// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  appendChild,
  createNode,
  destroyNode,
  focusedNode,
  getAttribute,
  insertBefore,
  layoutOf,
  scrollIntoViewNode,
  scrollNode,
  scrollOf,
  removeAttribute,
  removeChild,
  rootNodeId,
  setAttribute,
  setStyle,
  removeStyle,
} from "./tree.mts";
import type { EventLike } from "./types.mts";

// Ids come from the host, which numbers them in order.
let nextCallbackId = 0;

const native = {
  rootNodeId: vi.fn<() => number>(() => 42),
  createNode: vi.fn<(tag: string) => number>((_tag: string) => 1),
  appendChild: vi.fn<(parentId: number, childId: number) => void>(),
  insertBefore: vi.fn<(parentId: number, childId: number, anchorId: number | null) => void>(),
  removeChild: vi.fn<(parentId: number, childId: number) => void>(),
  setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  getAttribute: vi.fn<(nodeId: number, key: string) => string | number | boolean | null>(
    () => null,
  ),
  removeAttribute: vi.fn<(nodeId: number, key: string) => void>(),
  setStyle: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  removeStyle: vi.fn<(nodeId: number, key: string) => void>(),
  addEventListener: vi.fn<
    (nodeId: number, event: string, capture?: boolean, passive?: boolean) => number
  >(() => nextCallbackId++),
  removeEventListener: vi.fn<(nodeId: number, event: string, callbackId: number) => boolean>(
    () => true,
  ),
  destroyNode: vi.fn<(nodeId: number) => number[]>(() => []),
  focusNode: vi.fn<(nodeId: number) => void>(),
  blurNode: vi.fn<(nodeId: number) => void>(),
  focusedNode: vi.fn<() => number | null>(() => null),
  layoutOf: vi.fn<
    (nodeId: number) => readonly [number, number, number, number, number, number, number] | null
  >(() => null),
  scrollOf: vi.fn<(nodeId: number) => readonly [number, number, number, number] | null>(() => null),
  scrollNode: vi.fn<(nodeId: number, left: number | null, top: number | null) => void>(),
  scrollIntoViewNode: vi.fn<(nodeId: number, block: string, inline: string) => void>(),
  dispatchEvent: vi.fn<(nodeId: number, event: EventLike) => boolean>(() => true),
  click: vi.fn<(nodeId: number) => void>(),
};

beforeEach(() => {
  vi.clearAllMocks();
  nextCallbackId = 0;
  globalThis.__inca_native__ = native;
});

describe("wrapper functions forward to __inca_native__", () => {
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

  it("getAttribute", () => {
    native.getAttribute.mockReturnValueOnce("hello");
    expect(getAttribute(1, "label")).toBe("hello");
    expect(native.getAttribute).toHaveBeenCalledWith(1, "label");
  });

  it("layoutOf", () => {
    native.layoutOf.mockReturnValueOnce([1, 2, 3, 4, 0, 5, 6]);
    expect(layoutOf(1)).toEqual([1, 2, 3, 4, 0, 5, 6]);
    expect(native.layoutOf).toHaveBeenCalledWith(1);
  });

  it("scrollOf", () => {
    native.scrollOf.mockReturnValueOnce([1, 2, 3, 4]);
    expect(scrollOf(1)).toEqual([1, 2, 3, 4]);
    expect(native.scrollOf).toHaveBeenCalledWith(1);
  });

  it("scrollNode", () => {
    scrollNode(1, 5, null);
    expect(native.scrollNode).toHaveBeenCalledWith(1, 5, null);
  });

  it("scrollIntoViewNode", () => {
    scrollIntoViewNode(1, "center", "end");
    expect(native.scrollIntoViewNode).toHaveBeenCalledWith(1, "center", "end");
  });

  it("removeAttribute", () => {
    removeAttribute(1, "label");
    expect(native.removeAttribute).toHaveBeenCalledWith(1, "label");
  });

  it("destroyNode", () => {
    destroyNode(7);
    expect(native.destroyNode).toHaveBeenCalledWith(7);
  });

  it("removeStyle", () => {
    removeStyle(1, "display");
    expect(native.removeStyle).toHaveBeenCalledWith(1, "display");
  });

  it("setStyle", () => {
    setStyle(1, "display", "flex");
    expect(native.setStyle).toHaveBeenCalledWith(1, "display", "flex");
  });

  it("focusedNode", () => {
    native.focusedNode.mockReturnValueOnce(7);
    expect(focusedNode()).toBe(7);
  });
});
