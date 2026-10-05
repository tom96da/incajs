// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { blur, removeEventListener, setEventListener } from "./events.mts";
import { destroyNode } from "./tree.mts";

const native = {
  rootNodeId: vi.fn<() => number>(() => 0),
  createNode: vi.fn<(tag: string) => number>((_tag: string) => 1),
  appendChild: vi.fn<(parentId: number, childId: number) => void>(),
  insertBefore: vi.fn<(parentId: number, childId: number, anchorId: number | null) => void>(),
  removeChild: vi.fn<(parentId: number, childId: number) => void>(),
  setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  removeAttribute: vi.fn<(nodeId: number, key: string) => void>(),
  setStyle: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  removeStyle: vi.fn<(nodeId: number, key: string) => void>(),
  addEventListener: vi.fn<(nodeId: number, event: string, callbackId: number) => void>(),
  removeEventListener: vi.fn<(nodeId: number, event: string, callbackId: number) => boolean>(
    () => true,
  ),
  destroyNode: vi.fn<(nodeId: number) => number[]>(() => []),
  focusNode: vi.fn<(nodeId: number) => void>(),
  blurNode: vi.fn<(nodeId: number) => void>(),
};

beforeEach(() => {
  vi.clearAllMocks();
  globalThis.__inca_native__ = native;
  delete (globalThis as { __inca_callbacks__?: unknown }).__inca_callbacks__;
});

describe("setEventListener's callback registry", () => {
  it("keeps no listener behind when the native registration throws", () => {
    native.addEventListener.mockImplementationOnce(() => {
      throw new Error("node destroyed");
    });

    expect(() => setEventListener(1, "click", vi.fn())).toThrow("node destroyed");
    expect(Object.keys(globalThis.__inca_callbacks__)).toHaveLength(0);
  });

  it("stores the listener at __inca_callbacks__[id] and forwards that id natively", () => {
    const listener = vi.fn<() => void>();

    setEventListener(1, "click", listener);

    expect(native.addEventListener).toHaveBeenCalledTimes(1);
    const [nodeId, event, callbackId] = native.addEventListener.mock.calls[0]!;
    expect(nodeId).toBe(1);
    expect(event).toBe("click");
    const delivered = { type: "click" };
    globalThis.__inca_callbacks__[callbackId]!(delivered);
    expect(listener).toHaveBeenCalledExactlyOnceWith(delivered);
  });

  it("allocates a distinct id per registration", () => {
    setEventListener(1, "click", vi.fn());
    setEventListener(2, "click", vi.fn());

    const ids = Object.keys(globalThis.__inca_callbacks__);
    expect(ids).toHaveLength(2);
  });

  it("frees the previous callback id when the same (node, event) re-registers", () => {
    setEventListener(1, "click", vi.fn());
    const firstId = native.addEventListener.mock.calls[0]![2];

    setEventListener(1, "click", vi.fn());
    const secondId = native.addEventListener.mock.calls[1]![2];

    expect(secondId).not.toBe(firstId);
    expect(globalThis.__inca_callbacks__[firstId]).toBeUndefined();
    expect(Object.keys(globalThis.__inca_callbacks__)).toHaveLength(1);
  });
});

describe("removeEventListener", () => {
  it("drops both halves of the registration", () => {
    setEventListener(1, "click", vi.fn());
    const callbackId = native.addEventListener.mock.calls[0]![2];

    removeEventListener(1, "click");

    expect(native.removeEventListener).toHaveBeenCalledWith(1, "click", callbackId);
    expect(globalThis.__inca_callbacks__[callbackId]).toBeUndefined();
  });

  it("is a no-op when nothing is registered", () => {
    removeEventListener(999, "click");

    expect(native.removeEventListener).not.toHaveBeenCalled();
  });

  it("lets a re-registration allocate a fresh id", () => {
    setEventListener(1, "click", vi.fn());
    removeEventListener(1, "click");
    setEventListener(1, "click", vi.fn());

    expect(Object.keys(globalThis.__inca_callbacks__)).toHaveLength(1);
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

    expect(globalThis.__inca_callbacks__).toEqual({});
  });

  it("leaves another node's registration alone", () => {
    setEventListener(1, "click", vi.fn());
    setEventListener(2, "click", vi.fn());
    const survivor = native.addEventListener.mock.calls[1]![2];
    native.destroyNode.mockReturnValueOnce([native.addEventListener.mock.calls[0]![2]!]);

    destroyNode(1);

    expect(globalThis.__inca_callbacks__[survivor]).toBeDefined();
    expect(Object.keys(globalThis.__inca_callbacks__)).toHaveLength(1);
  });

  it("re-registering a destroyed node's (nodeId, event) does not resurrect the old id", () => {
    setEventListener(1, "click", vi.fn());
    const firstId = native.addEventListener.mock.calls[0]![2];
    native.destroyNode.mockReturnValueOnce([firstId!]);
    destroyNode(1);

    setEventListener(1, "click", vi.fn());

    expect(native.removeEventListener).not.toHaveBeenCalled();
    expect(Object.keys(globalThis.__inca_callbacks__)).toHaveLength(1);
  });
});

describe("latestEventId", () => {
  // A fresh module per test, so the tracker starts at 0.
  let tracked: typeof import("./events.mts");
  beforeEach(async () => {
    vi.resetModules();
    tracked = await import("./events.mts");
  });

  const fire = (event: unknown): void => {
    const id = native.addEventListener.mock.calls.at(-1)![2];
    globalThis.__inca_callbacks__[id]!(event);
  };

  it("tracks the highest eventId a listener has received, before it runs", () => {
    const seen: number[] = [];
    tracked.setEventListener(1, "click", () => seen.push(tracked.latestEventId()));
    expect(tracked.latestEventId()).toBe(0);
    fire({ eventId: 5 });
    fire({ eventId: 7 });
    fire({ eventId: 6 });

    expect(seen).toEqual([5, 7, 7]);
    expect(tracked.latestEventId()).toBe(7);
  });

  it("ignores an argument with no numeric eventId", () => {
    tracked.setEventListener(1, "click", vi.fn());

    for (const event of [undefined, null, 3, {}, { eventId: "9" }]) fire(event);

    expect(tracked.latestEventId()).toBe(0);
  });
});

describe("blur", () => {
  it("forwards the node id to native blurNode", () => {
    blur(7);
    expect(native.blurNode).toHaveBeenCalledWith(7);
  });
});
