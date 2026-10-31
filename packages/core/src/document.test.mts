// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { document } from "./document.mts";
import { HANDLER_EVENTS } from "./element/eventHandlers.mts";
import { registerElement, unregisterElement } from "./element/lifecycle.mts";
import { Document } from "./interfaces.mts";
import "./targets.mts";
import type { IncaElement } from "./element/types.mts";

let nextCallbackId = 0;
let focused: number | null = null;

const native = {
  focusedNode: vi.fn<() => number | null>(() => focused),
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
  focused = null;
  globalThis.__inca_native__ = native as never;
  delete (globalThis as { __inca_callbacks__?: unknown }).__inca_callbacks__;
});

afterEach(() => {
  unregisterElement(5);
});

describe("document", () => {
  it("is an instance of Document", () => {
    expect(document).toBeInstanceOf(Document);
  });

  it("has the Document string tag", () => {
    expect(Object.prototype.toString.call(document)).toBe("[object Document]");
  });

  it("reads the focused element as activeElement", () => {
    const handle = { __node_id: 5 } as unknown as IncaElement;
    registerElement(handle);
    const states = [];

    states.push(document.activeElement);
    focused = 5;
    states.push(document.activeElement);
    focused = null;
    states.push(document.activeElement);

    expect(states).toEqual([null, handle, null]);
  });

  it("reads null as activeElement for a focused id with no registered element", () => {
    focused = 999;

    expect(document.activeElement).toBeNull();
  });

  it("registers a listener on the document target", () => {
    document.addEventListener("keydown", vi.fn<() => void>());

    expect(native.addEventListener).toHaveBeenCalledExactlyOnceWith(
      "$document",
      "keydown",
      false,
      false,
    );
  });

  it("removes a listener from the document target", () => {
    const listener = vi.fn<() => void>();
    document.addEventListener("keydown", listener);
    const callbackId = native.addEventListener.mock.results[0]!.value;

    document.removeEventListener("keydown", listener);

    expect(native.removeEventListener).toHaveBeenCalledExactlyOnceWith(
      "$document",
      "keydown",
      callbackId,
    );
  });

  it("dispatches at the document target", () => {
    const event = { type: "ping" };

    document.dispatchEvent(event);

    expect(native.dispatchEvent).toHaveBeenCalledExactlyOnceWith("$document", event);
  });

  it("makes wheel listeners passive unless the options say otherwise", () => {
    document.addEventListener("wheel", vi.fn<() => void>());
    document.addEventListener("wheel", vi.fn<() => void>(), { passive: false });
    document.addEventListener("scroll", vi.fn<() => void>());

    expect(native.addEventListener.mock.calls.map((call) => call[3])).toEqual([true, false, false]);
  });

  it("carries every HANDLER_EVENTS property", () => {
    const missing = HANDLER_EVENTS.filter((event) => !(`on${event}` in document));

    expect(missing).toEqual([]);
  });

  it("runs an onkeydown handler as a listener of the document", () => {
    const keys: unknown[] = [];

    document.onkeydown = function (this: unknown, event: { key: string }) {
      keys.push(this, event.key);
    };
    const callbackId = native.addEventListener.mock.results[0]!.value;
    globalThis.__inca_callbacks__[callbackId]!({ key: "a", currentTarget: document });

    expect(native.addEventListener).toHaveBeenCalledExactlyOnceWith(
      "$document",
      "keydown",
      false,
      false,
    );
    expect(keys).toEqual([document, "a"]);
  });
});
