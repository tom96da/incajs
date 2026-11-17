// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { nativeStub } from "../tests/setup.mts";
import "./targets.mts";
import { document } from "./document.mts";
import { createIncaComment, createIncaFragment } from "./element/handle.mts";
import { markFreed, markRemoved } from "./element/lifecycle.mts";
import { createIncaText } from "./element/text.mts";
import { createEventTarget } from "./eventTarget.mts";
import { window } from "./window.mts";
import type { HostCallback } from "./ffi/index.mts";
import type { EventLike, IncaEventTarget } from "./types.mts";

// Ids come from the host, which numbers them in order.
let nextCallbackId = 0;

const native = {
  addEventListener: vi.fn<
    (
      nodeId: number,
      event: string,
      callback: HostCallback,
      capture?: boolean,
      passive?: boolean,
      signal?: unknown,
    ) => number | null
  >(() => nextCallbackId++),
  removeEventListener: vi.fn<(nodeId: number, event: string, callbackId: number) => boolean>(
    () => true,
  ),
  dispatchEvent: vi.fn<(nodeId: number, event: EventLike) => boolean>(() => true),
};

beforeEach(() => {
  vi.clearAllMocks();
  nextCallbackId = 0;
  globalThis.__inca_native__ = nativeStub(native as never);
});

// Flags of every host registration as [event, capture, passive].
const registered = (): unknown[][] =>
  native.addEventListener.mock.calls.map(([, event, , capture, passive]) => [
    event,
    capture,
    passive,
  ]);

describe("addEventListener", () => {
  it("registers on the node the resolver names at call time", () => {
    let node = 5;
    const target = createEventTarget(() => node);

    target.addEventListener("click", vi.fn<() => void>());
    node = 6;
    target.addEventListener("click", vi.fn<() => void>());

    expect(native.addEventListener.mock.calls.map((call) => call[0])).toEqual([5, 6]);
  });

  it("reads a boolean option as capture", () => {
    const target = createEventTarget(() => 1);

    target.addEventListener("a", vi.fn<() => void>(), true);
    target.addEventListener("b", vi.fn<() => void>(), false);
    target.addEventListener("c", vi.fn<() => void>());
    target.addEventListener("d", vi.fn<() => void>(), { capture: true, passive: true });

    expect(registered()).toEqual([
      ["a", true, false],
      ["b", false, false],
      ["c", false, false],
      ["d", true, true],
    ]);
  });

  it("makes the listed events passive unless the options say otherwise", () => {
    const target = createEventTarget(() => 1, ["wheel"]);

    target.addEventListener("wheel", vi.fn<() => void>());
    target.addEventListener("wheel", vi.fn<() => void>(), { capture: true });
    target.addEventListener("wheel", vi.fn<() => void>(), { passive: false });
    target.addEventListener("wheel", vi.fn<() => void>(), true);
    target.addEventListener("keydown", vi.fn<() => void>());

    expect(registered()).toEqual([
      ["wheel", false, true],
      ["wheel", true, true],
      ["wheel", false, false],
      ["wheel", true, true],
      ["keydown", false, false],
    ]);
  });

  it("leaves every event non-passive when the list is empty", () => {
    createEventTarget(() => 1).addEventListener("wheel", vi.fn<() => void>());

    expect(registered()).toEqual([["wheel", false, false]]);
  });

  it("ignores a function that is already registered for the type and capture", () => {
    const target = createEventTarget(() => 1);
    const listener = vi.fn<() => void>();

    target.addEventListener("click", listener);
    target.addEventListener("click", listener, false);
    target.addEventListener("click", listener, { capture: false });

    expect(native.addEventListener).toHaveBeenCalledTimes(1);
  });
});

describe("removeEventListener", () => {
  it("drops the registration of the function, matching capture in either form", () => {
    const target = createEventTarget(() => 1);
    const listener = vi.fn<() => void>();
    target.addEventListener("click", listener);
    target.addEventListener("click", listener, true);
    const [plainId, captureId] = native.addEventListener.mock.results.map((result) => result.value);

    target.removeEventListener("click", listener);
    target.removeEventListener("click", listener, { capture: true });

    expect(native.removeEventListener.mock.calls).toEqual([
      [1, "click", plainId],
      [1, "click", captureId],
    ]);
  });

  it("accepts a boolean capture", () => {
    const target = createEventTarget(() => 1);
    const listener = vi.fn<() => void>();
    target.addEventListener("click", listener, true);

    target.removeEventListener("click", listener, false);
    expect(native.removeEventListener).not.toHaveBeenCalled();

    target.removeEventListener("click", listener, true);
    expect(native.removeEventListener).toHaveBeenCalledTimes(1);
  });

  it("finds the registration of a passive default by function and capture", () => {
    const target = createEventTarget(() => 1, ["wheel"]);
    const listener = vi.fn<() => void>();
    target.addEventListener("wheel", listener);

    target.removeEventListener("wheel", listener);

    expect(native.removeEventListener).toHaveBeenCalledTimes(1);
  });
});

describe("dispatchEvent", () => {
  it("forwards the event to the node the resolver names and returns the host's answer", () => {
    const target = createEventTarget(() => 9);
    const event = { type: "ping" };
    native.dispatchEvent.mockReturnValueOnce(false);

    expect(target.dispatchEvent(event)).toBe(false);
    expect(native.dispatchEvent).toHaveBeenCalledExactlyOnceWith(9, event);
  });
});

describe("the listener methods of the nodes", () => {
  const kinds: [string, () => IncaEventTarget & { __node_id: number }][] = [
    ["a text node", () => createIncaText(71, "x")],
    ["a comment", () => createIncaComment(72)],
    ["a fragment", () => createIncaFragment(73)],
  ];

  it("run a listener of a text node for a dispatch at it, with the node as this", () => {
    const text = createIncaText(70, "x");
    const listener = vi.fn<(this: unknown) => void>();
    text.addEventListener("click", listener);
    const callback = native.addEventListener.mock.calls[0]![2];

    callback({ currentTarget: globalThis.__inca_targets__([70])[0] });

    expect(listener.mock.contexts[0]).toBe(text);
  });

  it.each(kinds)("ignore addEventListener on a removed node, %s", (_kind, make) => {
    const node = make();
    markRemoved([node.__node_id]);

    node.addEventListener("click", vi.fn<() => void>());

    expect(native.addEventListener).not.toHaveBeenCalled();
  });

  it.each(kinds)("ignore removeEventListener on a removed node, %s", (_kind, make) => {
    const node = make();
    const listener = vi.fn<() => void>();
    node.addEventListener("click", listener);
    markRemoved([node.__node_id]);

    node.removeEventListener("click", listener);

    expect(native.removeEventListener).not.toHaveBeenCalled();
  });

  it.each(kinds)(
    "answer true from dispatchEvent on a freed node with no call, %s",
    (_kind, make) => {
      const node = make();
      markFreed([node.__node_id]);

      expect(node.dispatchEvent(new Event("x"))).toBe(true);
      expect(native.dispatchEvent).not.toHaveBeenCalled();
    },
  );

  it.each([
    ["window", () => window],
    ["document", () => document],
  ] as const)("run addEventListener of %s through the host", (_name, make) => {
    make().addEventListener("click", vi.fn<() => void>());

    expect(native.addEventListener).toHaveBeenCalledOnce();
  });

  it.each([
    ["window", () => window],
    ["document", () => document],
  ] as const)("run removeEventListener of %s through the host", (_name, make) => {
    const listener = vi.fn<() => void>();
    make().addEventListener("click", listener);

    make().removeEventListener("click", listener);

    expect(native.removeEventListener).toHaveBeenCalledOnce();
  });

  it.each([
    ["window", () => window],
    ["document", () => document],
  ] as const)("run dispatchEvent of %s through the host", (_name, make) => {
    make().dispatchEvent(new Event("x"));

    expect(native.dispatchEvent).toHaveBeenCalledOnce();
  });
});

describe("a signal as the receiver", () => {
  it("registers, removes and dispatches on the signal itself", () => {
    const signal = new AbortSignal();
    const listener = vi.fn<() => void>();
    const event = { type: "abort" } as unknown as EventLike;

    signal.addEventListener("abort", listener);
    signal.dispatchEvent(event as never);
    signal.removeEventListener("abort", listener);

    expect(native.addEventListener.mock.calls[0]!.slice(0, 2)).toEqual([signal, "abort"]);
    expect(native.addEventListener.mock.calls[0]!.slice(3, 5)).toEqual([false, false]);
    expect(native.dispatchEvent).toHaveBeenCalledWith(signal, event);
    expect(native.removeEventListener.mock.calls[0]!.slice(0, 2)).toEqual([signal, "abort"]);
  });

  it("throws a TypeError for a plain object", () => {
    const call = () => document.addEventListener.call({}, "abort", () => undefined);

    expect(call).toThrow(TypeError);
  });
});

describe("addEventListener reads its options as properties", () => {
  const target = (): IncaEventTarget => createEventTarget(5);

  it("takes the signal of a class instance with a getter", () => {
    const signal = new AbortSignal();
    class Options {
      get signal() {
        return signal;
      }
    }

    target().addEventListener("click", () => undefined, new Options() as never);

    expect(native.addEventListener.mock.calls[0]![5]).toBe(signal);
  });

  it("throws a TypeError for an inherited signal that is no signal", () => {
    const options = Object.create({ signal: {} }) as never;

    expect(() => target().addEventListener("click", () => undefined, options)).toThrow(TypeError);
  });

  it("reads capture, once, passive and signal once each, in that order", () => {
    const reads: string[] = [];
    const options = {};
    for (const name of ["signal", "passive", "once", "capture"]) {
      Object.defineProperty(options, name, {
        get() {
          reads.push(name);
          return undefined;
        },
        enumerable: true,
      });
    }

    target().addEventListener("click", () => undefined, options as never);

    expect(reads).toEqual(["capture", "once", "passive", "signal"]);
  });

  it("takes an inherited capture", () => {
    const options = Object.create({ capture: true }) as never;

    target().addEventListener("click", () => undefined, options);

    expect(native.addEventListener.mock.calls[0]![3]).toBe(true);
  });

  it("takes an inherited once", () => {
    const options = Object.create({ once: true }) as never;
    target().addEventListener("click", () => undefined, options);

    native.addEventListener.mock.calls[0]![2]({});

    expect(native.removeEventListener).toHaveBeenCalledTimes(1);
  });
});
