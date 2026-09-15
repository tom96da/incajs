// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { removeEventListener, setEventListener } from "./events.mts";
import { destroyNode } from "./tree.mts";

const native = {
  rootNodeId: vi.fn<() => number>(() => 0),
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
  delete (globalThis as { __gpjsui_callbacks__?: unknown }).__gpjsui_callbacks__;
});

describe("setEventListener's callback registry", () => {
  it("stores the listener at __gpjsui_callbacks__[id] and forwards that id natively", () => {
    const listener = vi.fn<() => void>();

    setEventListener(1, "click", listener);

    expect(native.addEventListener).toHaveBeenCalledTimes(1);
    const [nodeId, event, callbackId] = native.addEventListener.mock.calls[0]!;
    expect(nodeId).toBe(1);
    expect(event).toBe("click");
    expect(globalThis.__gpjsui_callbacks__[callbackId]).toBe(listener);
  });

  it("allocates a distinct id per registration", () => {
    setEventListener(1, "click", vi.fn());
    setEventListener(2, "click", vi.fn());

    const ids = Object.keys(globalThis.__gpjsui_callbacks__);
    expect(ids).toHaveLength(2);
  });

  it("frees the previous callback id when the same (node, event) re-registers", () => {
    setEventListener(1, "click", vi.fn());
    const firstId = native.addEventListener.mock.calls[0]![2];

    setEventListener(1, "click", vi.fn());
    const secondId = native.addEventListener.mock.calls[1]![2];

    expect(secondId).not.toBe(firstId);
    expect(globalThis.__gpjsui_callbacks__[firstId]).toBeUndefined();
    expect(Object.keys(globalThis.__gpjsui_callbacks__)).toHaveLength(1);
  });
});

describe("removeEventListener", () => {
  it("drops both halves of the registration", () => {
    setEventListener(1, "click", vi.fn());
    const callbackId = native.addEventListener.mock.calls[0]![2];

    removeEventListener(1, "click");

    expect(native.removeEventListener).toHaveBeenCalledWith(1, "click", callbackId);
    expect(globalThis.__gpjsui_callbacks__[callbackId]).toBeUndefined();
  });

  it("is a no-op when nothing is registered", () => {
    removeEventListener(999, "click");

    expect(native.removeEventListener).not.toHaveBeenCalled();
  });

  it("lets a re-registration allocate a fresh id", () => {
    setEventListener(1, "click", vi.fn());
    removeEventListener(1, "click");
    setEventListener(1, "click", vi.fn());

    expect(Object.keys(globalThis.__gpjsui_callbacks__)).toHaveLength(1);
  });
});

describe("destroyNode", () => {
  it("frees the callback ids the host reports, including a descendant's", () => {
    setEventListener(1, "click", vi.fn());
    setEventListener(2, "click", vi.fn());
    const [parentCallback, childCallback] = native.addEventListener.mock.calls.map(
      (call) => call[2],
    );
    // The host frees the whole subtree, so node 2's id comes back from
    // destroying node 1.
    native.destroyNode.mockReturnValueOnce([parentCallback!, childCallback!]);

    destroyNode(1);

    expect(globalThis.__gpjsui_callbacks__).toEqual({});
  });

  it("leaves another node's registration alone", () => {
    setEventListener(1, "click", vi.fn());
    setEventListener(2, "click", vi.fn());
    const survivor = native.addEventListener.mock.calls[1]![2];
    native.destroyNode.mockReturnValueOnce([native.addEventListener.mock.calls[0]![2]!]);

    destroyNode(1);

    expect(globalThis.__gpjsui_callbacks__[survivor]).toBeDefined();
    expect(Object.keys(globalThis.__gpjsui_callbacks__)).toHaveLength(1);
  });

  it("re-registering a destroyed node's (nodeId, event) does not resurrect the old id", () => {
    setEventListener(1, "click", vi.fn());
    const firstId = native.addEventListener.mock.calls[0]![2];
    native.destroyNode.mockReturnValueOnce([firstId!]);
    destroyNode(1);

    setEventListener(1, "click", vi.fn());

    expect(native.removeEventListener).not.toHaveBeenCalled();
    expect(Object.keys(globalThis.__gpjsui_callbacks__)).toHaveLength(1);
  });
});
