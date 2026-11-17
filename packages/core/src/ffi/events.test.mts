// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import {
  addListener,
  blur,
  click,
  focus,
  focusedNode,
  removeListener,
  signalId,
  windowTargetId,
} from "./events.mts";
import type { HostCallback, WindowTargetId } from "./native.mts";

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
  >(() => 41),
  removeEventListener: vi.fn<(nodeId: number, event: string, callbackId: number) => boolean>(
    () => true,
  ),
  focusNode: vi.fn<(nodeId: number, preventScroll?: boolean) => void>(),
  blurNode: vi.fn<(nodeId: number) => void>(),
  focusedNode: vi.fn<() => number | null>(() => null),
  click: vi.fn<(nodeId: number) => void>(),
  windowTargetId: vi.fn<() => WindowTargetId>(() => "window-target" as WindowTargetId),
  signalId: vi.fn<(value: unknown) => number | null>(() => 7),
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.stubGlobal("__inca_native__", nativeStub(native));
});

describe("wrapper functions forward to __inca_native__", () => {
  it("addListener passes the options on and gives the callback id", () => {
    const callback = vi.fn<HostCallback>();

    expect(addListener(1, "click", callback, true, false)).toBe(41);
    expect(native.addEventListener).toHaveBeenCalledExactlyOnceWith(
      1,
      "click",
      callback,
      true,
      false,
      undefined,
    );
  });

  it("removeListener passes the callback id on and gives the host's answer", () => {
    native.removeEventListener.mockReturnValueOnce(false);

    expect(removeListener(1, "click", 41)).toBe(false);
    expect(native.removeEventListener).toHaveBeenCalledExactlyOnceWith(1, "click", 41);
  });

  it("windowTargetId asks the host once over two calls", () => {
    windowTargetId();
    windowTargetId();

    expect(native.windowTargetId).toHaveBeenCalledTimes(1);
  });

  it("focusedNode gives the id the host reports", () => {
    native.focusedNode.mockReturnValueOnce(7);
    expect(focusedNode()).toBe(7);
  });
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

describe("signalId", () => {
  it("gives the id the host answers for a value", () => {
    const value = {};

    expect(signalId(value)).toBe(7);
    expect(native.signalId).toHaveBeenCalledWith(value);
  });
});

describe("addListener with a signal", () => {
  it("forwards the signal as the sixth argument", () => {
    const signal = {} as never;

    addListener(1, "click", () => undefined, false, false, signal);

    expect(native.addEventListener.mock.calls[0]![5]).toBe(signal);
  });

  it("gives the null the host answers", () => {
    native.addEventListener.mockReturnValueOnce(null);

    expect(addListener(1, "click", () => undefined, false, false, {} as never)).toBeNull();
  });
});
