// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  addEventListener,
  blur,
  click,
  dispatchEvent,
  focus,
  focusedNode,
  registeredGroupCount,
  removeEventListener,
} from "./events.mts";
import { destroyNode } from "./tree.mts";
import type { EventLike } from "./types.mts";

const native = {
  rootNodeId: vi.fn<() => number>(() => 0),
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
  focusedNode: vi.fn<() => number | null>(() => null),
  layoutOf: vi.fn<
    (nodeId: number) => readonly [number, number, number, number, number, number, number] | null
  >(() => null),
  dispatchEvent: vi.fn<(nodeId: number, event: EventLike) => boolean>(() => true),
  scrollOf: vi.fn<(nodeId: number) => readonly [number, number, number, number] | null>(() => null),
  scrollNode: vi.fn<(nodeId: number, left: number | null, top: number | null) => void>(),
  scrollIntoViewNode: vi.fn<(nodeId: number, block: string, inline: string) => void>(),
  click: vi.fn<(nodeId: number) => void>(),
};

beforeEach(() => {
  vi.clearAllMocks();
  globalThis.__inca_native__ = native;
  delete (globalThis as { __inca_callbacks__?: unknown }).__inca_callbacks__;
});

describe("addEventListener's callback registry", () => {
  it("keeps no listener behind when the native registration throws", () => {
    native.addEventListener.mockImplementationOnce(() => {
      throw new Error("node destroyed");
    });
    const listener = vi.fn<() => void>();

    expect(() => addEventListener(1, "click", listener)).toThrow("node destroyed");
    expect(Object.keys(globalThis.__inca_callbacks__)).toHaveLength(0);

    addEventListener(1, "click", listener);
    expect(native.addEventListener).toHaveBeenCalledTimes(2);
  });

  it("stores the listener at __inca_callbacks__[id] and forwards that id natively", () => {
    const listener = vi.fn<() => void>();

    addEventListener(1, "click", listener);

    expect(native.addEventListener).toHaveBeenCalledTimes(1);
    const [nodeId, event, callbackId] = native.addEventListener.mock.calls[0]!;
    expect(nodeId).toBe(1);
    expect(event).toBe("click");
    const delivered = { type: "click" };
    globalThis.__inca_callbacks__[callbackId]!(delivered);
    expect(listener).toHaveBeenCalledExactlyOnceWith(delivered);
  });

  it("allocates a distinct id per registration", () => {
    addEventListener(1, "click", vi.fn<() => void>());
    addEventListener(2, "click", vi.fn<() => void>());

    expect(Object.keys(globalThis.__inca_callbacks__)).toHaveLength(2);
  });
});

describe("identity of a registration", () => {
  it("is node, event, capture and function", () => {
    const listener = vi.fn<() => void>();

    addEventListener(1, "click", listener);
    addEventListener(2, "click", listener);
    addEventListener(1, "keydown", listener);
    addEventListener(1, "click", listener, { capture: true });
    addEventListener(1, "click", vi.fn<() => void>());

    expect(native.addEventListener).toHaveBeenCalledTimes(5);
    expect(Object.keys(globalThis.__inca_callbacks__)).toHaveLength(5);
  });

  it("ignores a second registration of the same function, node, event and capture", () => {
    const listener = vi.fn<() => void>();
    addEventListener(1, "click", listener, { capture: true });

    addEventListener(1, "click", listener, { capture: true, passive: true, once: true });

    expect(native.addEventListener).toHaveBeenCalledTimes(1);
    expect(native.addEventListener).toHaveBeenCalledWith(
      1,
      "click",
      expect.any(Number),
      true,
      false,
    );
    expect(Object.keys(globalThis.__inca_callbacks__)).toHaveLength(1);
  });

  it("registers the same function again once its registration is gone", () => {
    const listener = vi.fn<() => void>();
    addEventListener(1, "click", listener);
    removeEventListener(1, "click", listener);

    addEventListener(1, "click", listener);

    expect(native.addEventListener).toHaveBeenCalledTimes(2);
    expect(Object.keys(globalThis.__inca_callbacks__)).toHaveLength(1);
  });
});

describe("registry groups", () => {
  // Fresh node ids and relative counts, since earlier tests leave registrations behind.
  let base = 0;
  beforeEach(() => {
    base = registeredGroupCount();
  });

  it("empties when every listener of several nodes is removed", () => {
    const listener = vi.fn<() => void>();
    for (const node of [9001, 9002, 9003]) {
      addEventListener(node, "click", listener);
      addEventListener(node, "click", listener, { capture: true });
    }
    expect(registeredGroupCount() - base).toBe(6);

    for (const node of [9001, 9002, 9003]) removeEventListener(node, "click", listener);
    expect(registeredGroupCount() - base).toBe(3);
    for (const node of [9001, 9002, 9003])
      removeEventListener(node, "click", listener, { capture: true });

    expect(registeredGroupCount() - base).toBe(0);
  });

  it("empties when the host reports destroyed nodes' registrations", () => {
    addEventListener(9001, "click", vi.fn<() => void>());
    addEventListener(9002, "keydown", vi.fn<() => void>());
    native.destroyNode.mockReturnValueOnce(native.addEventListener.mock.calls.map((c) => c[2]));

    destroyNode(9001);

    expect(registeredGroupCount() - base).toBe(0);
  });

  it("keeps a group while another listener of it remains", () => {
    const kept = vi.fn<() => void>();
    addEventListener(9001, "click", kept);
    addEventListener(9001, "click", vi.fn<() => void>());

    removeEventListener(9001, "click", kept);

    expect(registeredGroupCount() - base).toBe(1);
  });
});

describe("listener objects", () => {
  it("ignores a null listener on add and remove", () => {
    addEventListener(1, "click", null);
    removeEventListener(1, "click", null);

    expect(native.addEventListener).not.toHaveBeenCalled();
    expect(native.removeEventListener).not.toHaveBeenCalled();
  });

  it("calls handleEvent with the object as this", () => {
    const seen: unknown[] = [];
    const object = {
      handleEvent(this: unknown, event: unknown) {
        seen.push(this, event);
      },
    };
    addEventListener(1, "click", object);

    globalThis.__inca_callbacks__[native.addEventListener.mock.calls[0]![2]]!({ type: "click" });

    expect(seen).toEqual([object, { type: "click" }]);
  });

  it("calls the handleEvent present at dispatch, not the one present at registration", () => {
    const original = vi.fn<() => void>();
    const object = { handleEvent: original };
    const replaced = vi.fn<() => void>();
    addEventListener(1, "click", object);
    object.handleEvent = replaced;

    globalThis.__inca_callbacks__[native.addEventListener.mock.calls[0]![2]]!({ type: "click" });

    expect(original).not.toHaveBeenCalled();
    expect(replaced).toHaveBeenCalledTimes(1);
  });

  it("keys the registration on the object", () => {
    const object = { handleEvent: vi.fn<() => void>() };
    addEventListener(1, "click", object);
    addEventListener(1, "click", object);
    addEventListener(1, "click", { handleEvent: object.handleEvent });
    expect(native.addEventListener).toHaveBeenCalledTimes(2);

    removeEventListener(1, "click", object.handleEvent);
    expect(native.removeEventListener).not.toHaveBeenCalled();

    removeEventListener(1, "click", object);
    expect(native.removeEventListener).toHaveBeenCalledTimes(1);
  });

  it("runs a once object listener a single time", () => {
    const object = { handleEvent: vi.fn<() => void>() };
    addEventListener(1, "click", object, { once: true });
    const callbackId = native.addEventListener.mock.calls[0]![2];

    globalThis.__inca_callbacks__[callbackId]!();

    expect(object.handleEvent).toHaveBeenCalledTimes(1);
    expect(native.removeEventListener).toHaveBeenCalledExactlyOnceWith(1, "click", callbackId);
  });
});

describe("listener options", () => {
  it("forward capture and passive to the host, false when absent", () => {
    addEventListener(100, "a", vi.fn<() => void>());
    addEventListener(100, "b", vi.fn<() => void>(), { capture: true });
    addEventListener(100, "c", vi.fn<() => void>(), { passive: true });
    addEventListener(100, "d", vi.fn<() => void>(), { capture: true, passive: true });

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

  it("run a once listener a single time and free its registration before it runs", () => {
    const listener = vi.fn<() => void>(() => {
      expect(native.removeEventListener).toHaveBeenCalledTimes(1);
    });
    addEventListener(1, "click", listener, { once: true });
    const callbackId = native.addEventListener.mock.calls[0]![2];

    globalThis.__inca_callbacks__[callbackId]!({ type: "click" });

    expect(listener).toHaveBeenCalledExactlyOnceWith({ type: "click" });
    expect(native.removeEventListener).toHaveBeenCalledExactlyOnceWith(1, "click", callbackId);
    expect(globalThis.__inca_callbacks__[callbackId]).toBeUndefined();
  });

  it("let a once listener register itself again from inside its run", () => {
    const listener = vi.fn<() => void>(() =>
      addEventListener(1, "click", listener, { once: true }),
    );
    addEventListener(1, "click", listener, { once: true });

    globalThis.__inca_callbacks__[native.addEventListener.mock.calls[0]![2]]!();

    expect(native.addEventListener).toHaveBeenCalledTimes(2);
    expect(Object.keys(globalThis.__inca_callbacks__)).toHaveLength(1);
  });

  it("free a once listener of a capture registration through its own capture flag", () => {
    addEventListener(1, "click", vi.fn<() => void>(), { once: true, capture: true });
    const callbackId = native.addEventListener.mock.calls[0]![2];

    globalThis.__inca_callbacks__[callbackId]!();

    expect(native.removeEventListener).toHaveBeenCalledExactlyOnceWith(1, "click", callbackId);
  });
});

describe("removeEventListener", () => {
  it("drops both halves of the registration of that function", () => {
    const listener = vi.fn<() => void>();
    const other = vi.fn<() => void>();
    addEventListener(1, "click", listener);
    addEventListener(1, "click", other);
    const [listenerId, otherId] = native.addEventListener.mock.calls.map((call) => call[2]);

    removeEventListener(1, "click", listener);

    expect(native.removeEventListener).toHaveBeenCalledExactlyOnceWith(1, "click", listenerId);
    expect(globalThis.__inca_callbacks__[listenerId!]).toBeUndefined();
    expect(globalThis.__inca_callbacks__[otherId!]).toBeDefined();
  });

  it("selects the registration by capture", () => {
    const listener = vi.fn<() => void>();
    addEventListener(130, "click", listener);
    addEventListener(130, "click", listener, { capture: true });
    const [plainId, captureId] = native.addEventListener.mock.calls.map((call) => call[2]);

    removeEventListener(130, "click", listener, { capture: true });

    expect(native.removeEventListener).toHaveBeenCalledExactlyOnceWith(130, "click", captureId);
    expect(globalThis.__inca_callbacks__[plainId!]).toBeDefined();
    expect(globalThis.__inca_callbacks__[captureId!]).toBeUndefined();
  });

  it("changes nothing when no registration matches", () => {
    const listener = vi.fn<() => void>();
    addEventListener(1, "click", listener);

    removeEventListener(1, "click", listener, { capture: true });
    removeEventListener(1, "click", vi.fn<() => void>());
    removeEventListener(1, "keydown", listener);
    removeEventListener(2, "click", listener);
    removeEventListener(999, "click", listener);

    expect(native.removeEventListener).not.toHaveBeenCalled();
    expect(Object.keys(globalThis.__inca_callbacks__)).toHaveLength(1);
  });

  it("is a no-op when asked twice", () => {
    const listener = vi.fn<() => void>();
    addEventListener(1, "click", listener);

    removeEventListener(1, "click", listener);
    removeEventListener(1, "click", listener);

    expect(native.removeEventListener).toHaveBeenCalledTimes(1);
  });
});

describe("destroyNode", () => {
  it("frees the callback ids the host reports, including a descendant's", () => {
    addEventListener(1, "click", vi.fn<() => void>());
    addEventListener(2, "click", vi.fn<() => void>());
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
    addEventListener(1, "click", vi.fn<() => void>());
    addEventListener(2, "click", vi.fn<() => void>());
    const survivor = native.addEventListener.mock.calls[1]![2];
    native.destroyNode.mockReturnValueOnce([native.addEventListener.mock.calls[0]![2]!]);

    destroyNode(1);

    expect(globalThis.__inca_callbacks__[survivor]).toBeDefined();
    expect(Object.keys(globalThis.__inca_callbacks__)).toHaveLength(1);
  });

  it("registers a destroyed node's function afresh under a new id", () => {
    const listener = vi.fn<() => void>();
    addEventListener(1, "click", listener);
    const firstId = native.addEventListener.mock.calls[0]![2];
    native.destroyNode.mockReturnValueOnce([firstId!]);
    destroyNode(1);

    addEventListener(1, "click", listener);

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
    tracked.addEventListener(1, "click", () => seen.push(tracked.latestEventId()));
    expect(tracked.latestEventId()).toBe(0);
    fire({ eventId: 5 });
    fire({ eventId: 7 });
    fire({ eventId: 6 });

    expect(seen).toEqual([5, 7, 7]);
    expect(tracked.latestEventId()).toBe(7);
  });

  it("ignores an argument with no numeric eventId", () => {
    tracked.addEventListener(1, "click", vi.fn<() => void>());

    for (const event of [undefined, null, 3, {}, { eventId: "9" }]) fire(event);

    expect(tracked.latestEventId()).toBe(0);
  });
});

describe("focus", () => {
  it("passes no second argument when preventScroll is omitted", () => {
    focus(7);
    expect(native.focusNode).toHaveBeenCalledExactlyOnceWith(7);
  });

  it("passes the preventScroll flag through", () => {
    focus(7, true);
    focus(7, false);
    expect(native.focusNode).toHaveBeenNthCalledWith(1, 7, true);
    expect(native.focusNode).toHaveBeenNthCalledWith(2, 7, false);
  });
});

describe("blur", () => {
  it("forwards the node id to native blurNode", () => {
    blur(7);
    expect(native.blurNode).toHaveBeenCalledWith(7);
  });
});

describe("focusedNode", () => {
  it("returns the id native reports", () => {
    native.focusedNode.mockReturnValueOnce(7);
    expect(focusedNode()).toBe(7);
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
