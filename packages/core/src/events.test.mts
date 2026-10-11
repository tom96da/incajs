// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { blur, click, dispatchEvent, removeEventListener, setEventListener } from "./events.mts";
import { destroyNode } from "./tree.mts";
import type { EventLike } from "./types.mts";

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
  addEventListener:
    vi.fn<
      (
        nodeId: number,
        event: string,
        callbackId: number,
        capture?: boolean,
        passive?: boolean,
      ) => void
    >(),
  removeEventListener: vi.fn<(nodeId: number, event: string, callbackId: number) => boolean>(
    () => true,
  ),
  destroyNode: vi.fn<(nodeId: number) => number[]>(() => []),
  focusNode: vi.fn<(nodeId: number) => void>(),
  blurNode: vi.fn<(nodeId: number) => void>(),
  dispatchEvent: vi.fn<(nodeId: number, event: EventLike) => boolean>(() => true),
  click: vi.fn<(nodeId: number) => void>(),
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

describe("listener options", () => {
  it("forward capture and passive to the host, false when absent", () => {
    setEventListener(100, "a", vi.fn());
    setEventListener(100, "b", vi.fn(), { capture: true });
    setEventListener(100, "c", vi.fn(), { passive: true });
    setEventListener(100, "d", vi.fn(), { capture: true, passive: true });

    const flags = native.addEventListener.mock.calls.map(([, event, , capture, passive]) => [
      event,
      capture,
      passive,
    ]);
    expect(flags).toEqual([
      ["a", false, false],
      ["b", true, false],
      ["c", false, true],
      ["d", true, true],
    ]);
  });

  it("keep a bubble, a capture and a passive listener of one node and event apart", () => {
    setEventListener(110, "click", vi.fn());
    setEventListener(110, "click", vi.fn(), { capture: true });
    setEventListener(110, "click", vi.fn(), { passive: true });

    expect(native.removeEventListener).not.toHaveBeenCalled();
    expect(Object.keys(globalThis.__inca_callbacks__)).toHaveLength(3);
  });

  it("replace the listener of the same node, event, capture and passive", () => {
    setEventListener(120, "click", vi.fn(), { capture: true });
    const firstId = native.addEventListener.mock.calls[0]![2];

    setEventListener(120, "click", vi.fn(), { capture: true });

    expect(native.removeEventListener).toHaveBeenCalledExactlyOnceWith(120, "click", firstId);
    expect(Object.keys(globalThis.__inca_callbacks__)).toHaveLength(1);
  });

  it("select the registration that removeEventListener drops", () => {
    setEventListener(130, "click", vi.fn());
    setEventListener(130, "click", vi.fn(), { capture: true });
    const [plainId, captureId] = native.addEventListener.mock.calls.map((call) => call[2]);

    removeEventListener(130, "click", { capture: true });

    expect(native.removeEventListener).toHaveBeenCalledExactlyOnceWith(130, "click", captureId);
    expect(globalThis.__inca_callbacks__[plainId!]).toBeDefined();
    expect(globalThis.__inca_callbacks__[captureId!]).toBeUndefined();
  });

  it("leave a registration of other options alone when none matches", () => {
    setEventListener(140, "click", vi.fn());

    removeEventListener(140, "click", { capture: true });

    expect(native.removeEventListener).not.toHaveBeenCalled();
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

describe("dispatchEvent", () => {
  it("forwards the node and event to native and returns its answer", () => {
    const event = { type: "ping" };
    native.dispatchEvent.mockReturnValueOnce(false);

    expect(dispatchEvent(7, event)).toBe(false);
    expect(native.dispatchEvent).toHaveBeenCalledExactlyOnceWith(7, event);
  });
});

describe("click", () => {
  it("forwards the node id to native click", () => {
    click(7);
    expect(native.click).toHaveBeenCalledExactlyOnceWith(7);
  });
});
