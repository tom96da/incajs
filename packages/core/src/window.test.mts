// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { window } from "./window.mts";

// Ids come from the host, which numbers them in order.
let nextCallbackId = 0;

const native = {
  addEventListener: vi.fn<
    (target: number | string, event: string, capture?: boolean, passive?: boolean) => number
  >(() => nextCallbackId++),
  removeEventListener: vi.fn<
    (target: number | string, event: string, callbackId: number) => boolean
  >(() => true),
  dispatchEvent: vi.fn<(target: number | string, event: unknown) => boolean>(() => true),
};

beforeEach(() => {
  vi.clearAllMocks();
  nextCallbackId = 0;
  globalThis.__inca_native__ = native as never;
  delete (globalThis as { __inca_callbacks__?: unknown }).__inca_callbacks__;
});

describe("window", () => {
  it("listens on the window target", () => {
    const listener = vi.fn<() => void>();

    window.addEventListener("keydown", listener);
    const callbackId = native.addEventListener.mock.results[0]!.value;
    window.removeEventListener("keydown", listener);

    expect(native.addEventListener).toHaveBeenCalledWith("$window", "keydown", false, false);
    expect(native.removeEventListener).toHaveBeenCalledExactlyOnceWith(
      "$window",
      "keydown",
      callbackId,
    );
  });

  it("dispatches at the window target", () => {
    const event = { type: "ping" };

    window.dispatchEvent(event);

    expect(native.dispatchEvent).toHaveBeenCalledExactlyOnceWith("$window", event);
  });

  it("takes a listener that declares the event fields it reads", () => {
    const keys: string[] = [];
    window.addEventListener("keydown", (event: { key: string }) => keys.push(event.key));
    const callbackId = native.addEventListener.mock.results[0]!.value;

    globalThis.__inca_callbacks__[callbackId]!({ key: "a" });

    expect(keys).toEqual(["a"]);
  });

  it("makes wheel listeners passive unless the options say otherwise", () => {
    window.addEventListener("wheel", vi.fn<() => void>());
    window.addEventListener("wheel", vi.fn<() => void>(), { passive: false });
    window.addEventListener("scroll", vi.fn<() => void>());

    expect(native.addEventListener.mock.calls.map((call) => call[3])).toEqual([true, false, false]);
  });

  it("runs an onfocus handler as a listener of the window", () => {
    const seen: unknown[] = [];

    window.onfocus = function (this: unknown) {
      seen.push(this);
    };
    const callbackId = native.addEventListener.mock.results[0]!.value;
    globalThis.__inca_callbacks__[callbackId]!({ currentTarget: window });

    expect(native.addEventListener).toHaveBeenCalledExactlyOnceWith(
      "$window",
      "focus",
      false,
      false,
    );
    expect(seen).toEqual([window]);
  });
});
