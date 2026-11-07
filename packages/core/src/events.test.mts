// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { callbackIdFor, groupOf, registeredGroupCount } from "./callbacks.mts";
import { addEventListener, blur, click, focus, removeEventListener } from "./events.mts";
import { releaseListeners } from "./tree.mts";
import type { HostCallback, NodeLink } from "./native.mts";
import type { EventLike } from "./types.mts";

// Ids come from the host, which numbers them in order.
let nextCallbackId = 0;

const native = {
  rootNodeId: vi.fn<() => number>(() => 0),
  createNode: vi.fn<(tag: string) => number>((_tag: string) => 1),
  appendChild: vi.fn<(parentId: number, childId: number) => number>(() => 0),
  insertBefore: vi.fn<(parentId: number, childId: number, refId: number | null) => number>(() => 0),
  removeChild: vi.fn<(parentId: number, childId: number) => void>(),
  replaceChild: vi.fn<(parentId: number, nodeId: number, childId: number) => number>(() => 0),
  setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  getAttribute: vi.fn<(nodeId: number, key: string) => string | number | boolean | null>(
    () => null,
  ),
  removeAttribute: vi.fn<(nodeId: number, key: string) => void>(),
  setStyle: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  removeStyle: vi.fn<(nodeId: number, key: string) => void>(),
  inputValue: vi.fn<(nodeId: number) => string>(() => ""),
  setInputValue: vi.fn<(nodeId: number, value: string) => void>(),
  addEventListener: vi.fn<
    (
      nodeId: number,
      event: string,
      callback: HostCallback,
      capture?: boolean,
      passive?: boolean,
    ) => number
  >(() => nextCallbackId++),
  removeEventListener: vi.fn<(nodeId: number, event: string, callbackId: number) => boolean>(
    () => true,
  ),
  releaseListeners: vi.fn<(nodeId: number) => number[]>(() => []),
  attributeNames: vi.fn<(nodeId: number) => string[]>(() => []),
  matches: vi.fn<(nodeId: number, selector: string) => boolean>(() => false),
  closest: vi.fn<(nodeId: number, selector: string) => number | null>(() => null),
  checkSelector: vi.fn<(selector: string) => void>(),
  querySelector: vi.fn<(nodeId: number, selector: string) => number | null>(() => null),
  querySelectorAll: vi.fn<(nodeId: number, selector: string) => number[]>(() => []),
  holdNode: vi.fn<(nodeId: number) => void>(),
  releaseNode: vi.fn<(nodeId: number) => void>(),
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
  setPointerCapture: vi.fn<(nodeId: number, pointerId: number) => void>(),
  releasePointerCapture: vi.fn<(nodeId: number, pointerId: number) => void>(),
  hasPointerCapture: vi.fn<(nodeId: number, pointerId: number) => boolean>(),
  linkedNode: vi.fn<(nodeId: number, link: NodeLink) => number | null>(() => null),
  childNodeIds: vi.fn<(nodeId: number) => number[]>(() => []),
  isConnected: vi.fn<(nodeId: number) => boolean>(() => true),
  textContent: vi.fn<(nodeId: number) => string>(() => ""),
  wholeText: vi.fn<(nodeId: number) => string>(() => ""),
  substringData: vi.fn<(nodeId: number, offset: number, count: number) => string>(() => ""),
  replaceData: vi.fn<(nodeId: number, offset: number, count: number, data: string) => void>(),
  splitText: vi.fn<(nodeId: number, offset: number) => number>(() => 0),
};

beforeEach(() => {
  vi.clearAllMocks();
  nextCallbackId = 0;
  globalThis.__inca_native__ = native;
});

/** The function the host received for the registration at `index`. */
const received = (index = 0): HostCallback => native.addEventListener.mock.calls.at(index)![2];

describe("addEventListener's callback registry", () => {
  it("records nothing when the native call throws, and registers on the next call", () => {
    native.addEventListener.mockImplementationOnce(() => {
      throw new Error("node destroyed");
    });
    const listener = vi.fn<() => void>();

    expect(() => addEventListener(1, "click", listener)).toThrow("node destroyed");
    expect(callbackIdFor(groupOf(1, "click", false), listener)).toBeUndefined();

    addEventListener(1, "click", listener);
    expect(native.addEventListener).toHaveBeenCalledTimes(2);
    expect(callbackIdFor(groupOf(1, "click", false), listener)).toBe(0);
  });

  it("records the id the host returns", () => {
    native.addEventListener.mockReturnValueOnce(41);
    const listener = vi.fn<() => void>();

    addEventListener(1, "click", listener);

    expect(callbackIdFor(groupOf(1, "click", false), listener)).toBe(41);
  });

  it("forwards the recorded id on removal", () => {
    native.addEventListener.mockReturnValueOnce(41);
    const listener = vi.fn<() => void>();

    addEventListener(1, "click", listener);
    removeEventListener(1, "click", listener);
    expect(native.removeEventListener).toHaveBeenCalledExactlyOnceWith(1, "click", 41);
    expect(callbackIdFor(groupOf(1, "click", false), listener)).toBeUndefined();
  });

  it("passes the listener to the host as the callback", () => {
    const listener = vi.fn<() => void>();

    addEventListener(1, "click", listener);

    expect(native.addEventListener).toHaveBeenCalledTimes(1);
    const [nodeId, event] = native.addEventListener.mock.calls[0]!;
    expect(nodeId).toBe(1);
    expect(event).toBe("click");
    const delivered = { type: "click" };
    received()(delivered);
    expect(listener).toHaveBeenCalledExactlyOnceWith(delivered);
  });
});

describe("identity of a registration", () => {
  it("is target, event, capture and function", () => {
    const listener = vi.fn<() => void>();

    addEventListener(1, "click", listener);
    addEventListener(2, "click", listener);
    addEventListener(1, "keydown", listener);
    addEventListener(1, "click", listener, { capture: true });
    addEventListener(1, "click", vi.fn<() => void>());

    expect(native.addEventListener).toHaveBeenCalledTimes(5);
  });

  it("keeps the registrations of document and window apart", () => {
    const listener = vi.fn<() => void>();

    addEventListener("$document", "click", listener);
    addEventListener("$window", "click", listener);

    expect(native.addEventListener.mock.calls.map((call) => call[0])).toEqual([
      "$document",
      "$window",
    ]);
  });

  it("ignores a second registration of the same function, node, event and capture", () => {
    const listener = vi.fn<() => void>();
    addEventListener(1, "click", listener, { capture: true });

    addEventListener(1, "click", listener, { capture: true, passive: true, once: true });

    expect(native.addEventListener).toHaveBeenCalledTimes(1);
    expect(native.addEventListener).toHaveBeenCalledWith(
      1,
      "click",
      expect.any(Function),
      true,
      false,
    );
  });

  it("registers the same function again once its registration is gone", () => {
    const listener = vi.fn<() => void>();
    addEventListener(1, "click", listener);
    removeEventListener(1, "click", listener);

    addEventListener(1, "click", listener);

    expect(native.addEventListener).toHaveBeenCalledTimes(2);
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

  it("empties when the host reports released registrations", () => {
    addEventListener(9001, "click", vi.fn<() => void>());
    addEventListener(9002, "keydown", vi.fn<() => void>());
    native.releaseListeners.mockReturnValueOnce(
      native.addEventListener.mock.results.map((result) => result.value),
    );

    releaseListeners(9001);

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
  it("ignores a null listener on add", () => {
    addEventListener(1, "click", null);

    expect(native.addEventListener).not.toHaveBeenCalled();
  });

  it("calls handleEvent with the object as this", () => {
    const seen: unknown[] = [];
    const object = {
      handleEvent(this: unknown, event: unknown) {
        seen.push(this, event);
      },
    };
    addEventListener(1, "click", object);

    received()({
      type: "click",
    });

    expect(seen).toEqual([object, { type: "click" }]);
  });

  it("calls a listener function with the current target as this", () => {
    const seen: unknown[] = [];
    addEventListener(1, "click", function (this: unknown) {
      seen.push(this);
    });
    const currentTarget = { node: 1 };

    received()({
      type: "click",
      currentTarget,
    });

    expect(seen[0]).toBe(currentTarget);
  });

  it("calls a function listener with this undefined without a current target", () => {
    const seen: unknown[] = [];
    addEventListener(1, "click", function (this: unknown) {
      seen.push(this);
    });
    const call = received();

    call();
    call(null);
    call({ type: "click" });

    expect(seen).toEqual([undefined, undefined, undefined]);
  });

  it("calls the handleEvent present at dispatch, not the one present at registration", () => {
    const original = vi.fn<() => void>();
    const object = { handleEvent: original };
    const replaced = vi.fn<() => void>();
    addEventListener(1, "click", object);
    object.handleEvent = replaced;

    received()({
      type: "click",
    });

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
    const callbackId = native.addEventListener.mock.results[0]!.value;

    received()({ type: "click" });

    expect(listener).toHaveBeenCalledExactlyOnceWith({ type: "click" });
    expect(native.removeEventListener).toHaveBeenCalledExactlyOnceWith(1, "click", callbackId);
  });

  it("let a once listener register itself again from inside its run", () => {
    const listener = vi.fn<() => void>(() =>
      addEventListener(1, "click", listener, { once: true }),
    );
    addEventListener(1, "click", listener, { once: true });

    received()();

    expect(native.addEventListener).toHaveBeenCalledTimes(2);
  });

  it("free a once listener of a capture registration through its own capture flag", () => {
    addEventListener(1, "click", vi.fn<() => void>(), { once: true, capture: true });
    const callbackId = native.addEventListener.mock.results[0]!.value;

    received()();

    expect(native.removeEventListener).toHaveBeenCalledExactlyOnceWith(1, "click", callbackId);
  });
});

describe("removeEventListener", () => {
  it("forgets the registration of that function only", () => {
    const listener = vi.fn<() => void>();
    const other = vi.fn<() => void>();
    addEventListener(1, "click", listener);
    addEventListener(1, "click", other);
    const [listenerId, otherId] = native.addEventListener.mock.results.map(
      (result) => result.value,
    );

    removeEventListener(1, "click", listener);

    expect(native.removeEventListener).toHaveBeenCalledExactlyOnceWith(1, "click", listenerId);
    expect(callbackIdFor(groupOf(1, "click", false), listener)).toBeUndefined();
    expect(callbackIdFor(groupOf(1, "click", false), other)).toBe(otherId);
  });

  it("selects the registration by capture", () => {
    const listener = vi.fn<() => void>();
    addEventListener(130, "click", listener);
    addEventListener(130, "click", listener, { capture: true });
    const [plainId, captureId] = native.addEventListener.mock.results.map((result) => result.value);

    removeEventListener(130, "click", listener, { capture: true });

    expect(native.removeEventListener).toHaveBeenCalledExactlyOnceWith(130, "click", captureId);
    expect(callbackIdFor(groupOf(130, "click", false), listener)).toBe(plainId);
    expect(callbackIdFor(groupOf(130, "click", true), listener)).toBeUndefined();
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
    expect(callbackIdFor(groupOf(1, "click", false), listener)).toBeDefined();
  });
});

describe("releaseListeners", () => {
  it("forgets the callback ids the host reports", () => {
    const [releasedListener, keptListener] = [vi.fn<() => void>(), vi.fn<() => void>()];
    addEventListener(1, "click", releasedListener);
    addEventListener(2, "click", keptListener);
    const [first, second] = native.addEventListener.mock.results.map((result) => result.value);
    native.releaseListeners.mockReturnValueOnce([first!]);

    releaseListeners(1);

    expect(native.releaseListeners).toHaveBeenCalledWith(1);
    expect(callbackIdFor(groupOf(1, "click", false), releasedListener)).toBeUndefined();
    expect(callbackIdFor(groupOf(2, "click", false), keptListener)).toBe(second);
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
    received(-1)(event);
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

  it.each([[undefined], [null], [3], [{}], [{ eventId: "9" }]])(
    "ignores an argument with no numeric eventId (%o)",
    (event) => {
      tracked.addEventListener(1, "click", vi.fn<() => void>());

      fire(event);

      expect(tracked.latestEventId()).toBe(0);
    },
  );
});

describe("focus", () => {
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

describe("click", () => {
  it("forwards the node id to native click", () => {
    click(7);
    expect(native.click).toHaveBeenCalledExactlyOnceWith(7);
  });
});
