// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { window } from "./window.mts";

const native = {
  rootNodeId: vi.fn<() => number>(() => 77),
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
  dispatchEvent: vi.fn<(nodeId: number, event: unknown) => boolean>(() => true),
};

beforeEach(() => {
  vi.clearAllMocks();
  globalThis.__inca_native__ = native as never;
  delete (globalThis as { __inca_callbacks__?: unknown }).__inca_callbacks__;
});

describe("window", () => {
  it("listens on the root node", () => {
    const listener = vi.fn<() => void>();

    window.addEventListener("keydown", listener);
    const callbackId = native.addEventListener.mock.calls[0]![2];
    window.removeEventListener("keydown", listener);

    expect(native.addEventListener).toHaveBeenCalledWith(77, "keydown", callbackId, false, false);
    expect(native.removeEventListener).toHaveBeenCalledExactlyOnceWith(77, "keydown", callbackId);
  });

  it("dispatches at the root node", () => {
    const event = { type: "ping" };

    window.dispatchEvent(event);

    expect(native.dispatchEvent).toHaveBeenCalledExactlyOnceWith(77, event);
  });

  it("looks the root up each time a method runs", () => {
    window.addEventListener("keydown", vi.fn<() => void>());
    native.rootNodeId.mockReturnValueOnce(78);
    window.addEventListener("keyup", vi.fn<() => void>());

    expect(native.addEventListener.mock.calls.map((call) => call[0])).toEqual([77, 78]);
  });

  it("takes a listener that declares the event fields it reads", () => {
    const keys: string[] = [];
    window.addEventListener("keydown", (event: { key: string }) => keys.push(event.key));
    const callbackId = native.addEventListener.mock.calls[0]![2];

    globalThis.__inca_callbacks__[callbackId]!({ key: "a" });

    expect(keys).toEqual(["a"]);
  });

  it("makes wheel listeners passive unless the options say otherwise", () => {
    window.addEventListener("wheel", vi.fn<() => void>());
    window.addEventListener("wheel", vi.fn<() => void>(), { passive: false });
    window.addEventListener("scroll", vi.fn<() => void>());

    expect(native.addEventListener.mock.calls.map((call) => call[4])).toEqual([true, false, false]);
  });
});
