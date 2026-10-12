// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { createEventTarget } from "./eventTarget.mts";
import type { EventLike } from "./types.mts";

// Ids come from the host, which numbers them in order.
let nextCallbackId = 0;

const native = {
  addEventListener: vi.fn<
    (nodeId: number, event: string, capture?: boolean, passive?: boolean) => number
  >(() => nextCallbackId++),
  removeEventListener: vi.fn<(nodeId: number, event: string, callbackId: number) => boolean>(
    () => true,
  ),
  dispatchEvent: vi.fn<(nodeId: number, event: EventLike) => boolean>(() => true),
};

beforeEach(() => {
  vi.clearAllMocks();
  nextCallbackId = 0;
  globalThis.__inca_native__ = native as never;
  delete (globalThis as { __inca_callbacks__?: unknown }).__inca_callbacks__;
});

// Flags of every host registration as [event, capture, passive].
const registered = (): unknown[][] =>
  native.addEventListener.mock.calls.map(([, event, capture, passive]) => [
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
  it("drops the registration of the function, matching capture as a boolean or an object", () => {
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
