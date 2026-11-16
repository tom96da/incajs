// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { collectGarbage } from "../tests/gc.mts";
import { nativeStub } from "../tests/setup.mts";
import "./document.mts";
import "./abortSignal.mts";
import { signalTargetOf } from "./signalTarget.mts";
import type { HostCallback } from "./ffi/index.mts";

interface Registration {
  target: unknown;
  event: string;
  callback: HostCallback;
  removed: boolean;
}

let registrations: Registration[] = [];

const native = {
  addEventListener: vi.fn<(target: unknown, event: string, callback: HostCallback) => number>(
    (target, event, callback) => {
      registrations.push({ target, event, callback, removed: false });
      return registrations.length;
    },
  ),
  removeEventListener: vi.fn<(target: unknown, event: string, id: number) => boolean>(
    (_target, _event, id) => {
      registrations[id - 1]!.removed = true;
      return true;
    },
  ),
};

beforeEach(() => {
  vi.clearAllMocks();
  registrations = [];
  globalThis.__inca_native__ = nativeStub(native as never);
});

/** Runs the listeners of `signal` for `abort`, as the host walk does. */
const abort = (signal: AbortSignal): void => {
  for (const r of registrations.filter((x) => x.target === signal && !x.removed)) {
    r.callback({ currentTarget: signal, eventId: 1 });
  }
};

describe("onabort", () => {
  it("starts null and reads back an assigned function", () => {
    const signal = new AbortSignal();
    const handler = vi.fn<() => void>();

    const before = signal.onabort;
    signal.onabort = handler;

    expect([before, signal.onabort]).toEqual([null, handler]);
  });

  it("runs the handler at the abort with the signal as this", () => {
    const signal = new AbortSignal();
    const seen: unknown[] = [];
    signal.onabort = function (this: unknown) {
      seen.push(this);
    };

    abort(signal);

    expect(seen).toEqual([signal]);
  });

  it.each([null, undefined, 5, "x"])("removes the handler when %j is assigned", (value) => {
    const signal = new AbortSignal();
    const handler = vi.fn<() => void>();
    signal.onabort = handler;

    signal.onabort = value as never;
    abort(signal);

    expect([signal.onabort, handler.mock.calls.length]).toEqual([null, 0]);
  });

  it("keeps the position among the listeners when a second function is assigned", () => {
    const signal = new AbortSignal();
    const order: string[] = [];
    signal.addEventListener("abort", () => order.push("a"));
    signal.onabort = () => order.push("first");
    signal.addEventListener("abort", () => order.push("b"));

    signal.onabort = () => order.push("second");
    abort(signal);

    expect(order).toEqual(["a", "second", "b"]);
    expect(native.addEventListener).toHaveBeenCalledTimes(3);
  });

  it("reads an assigned object back and never calls it", () => {
    const signal = new AbortSignal();
    const handleEvent = vi.fn<() => void>();
    const object = { handleEvent };

    signal.onabort = object as never;
    abort(signal);

    expect([signal.onabort, handleEvent.mock.calls.length]).toEqual([object, 0]);
  });

  it("runs the same function twice as onabort and as a listener", () => {
    const signal = new AbortSignal();
    const handler = vi.fn<() => void>();
    signal.onabort = handler;
    signal.addEventListener("abort", handler);

    abort(signal);

    expect(handler).toHaveBeenCalledTimes(2);
  });

  it("registers at the end of the list after the handler was cleared", () => {
    const signal = new AbortSignal();
    const order: string[] = [];
    signal.onabort = () => order.push("old");
    signal.addEventListener("abort", () => order.push("listener"));
    signal.onabort = null;

    signal.onabort = () => order.push("new");
    abort(signal);

    expect(order).toEqual(["listener", "new"]);
  });

  it.each(["get", "set"])(
    "throws a TypeError from the %s accessor for a receiver that is no signal",
    (kind) => {
      const descriptor = Object.getOwnPropertyDescriptor(AbortSignal.prototype, "onabort")!;
      const call = (receiver: object) => () =>
        kind === "get" ? descriptor.get!.call(receiver) : descriptor.set!.call(receiver, null);

      expect(call({})).toThrow(TypeError);
      expect(call(Object.create(AbortSignal.prototype) as object)).toThrow(TypeError);
    },
  );

  it("registers its listener whatever a script does to addEventListener", () => {
    const parent = Object.getPrototypeOf(AbortSignal.prototype) as { addEventListener: unknown };
    const original = Object.getOwnPropertyDescriptor(parent, "addEventListener")!;
    const replaced = vi.fn<() => void>();
    Object.defineProperty(parent, "addEventListener", { ...original, value: replaced });
    const handler = vi.fn<() => void>();
    try {
      const signal = new AbortSignal();
      signal.onabort = handler;
      abort(signal);
    } finally {
      Object.defineProperty(parent, "addEventListener", original);
    }

    expect([handler.mock.calls.length, replaced.mock.calls.length]).toEqual([1, 0]);
  });

  it("is an enumerable, configurable accessor of the prototype with no own copy on a signal", () => {
    const descriptor = Object.getOwnPropertyDescriptor(AbortSignal.prototype, "onabort")!;

    expect([
      descriptor.enumerable,
      descriptor.configurable,
      descriptor.get?.name,
      descriptor.set?.name,
      Object.hasOwn(new AbortSignal(), "onabort"),
    ]).toEqual([true, true, "get onabort", "set onabort", false]);
  });

  it("leaves the real prototype alone when no host exists", async () => {
    const Real = (new AbortController().signal as unknown as { constructor: typeof AbortSignal })
      .constructor;
    const before = Object.getOwnPropertyDescriptor(Real.prototype, "onabort");
    const stand = globalThis.AbortSignal;
    const host = globalThis.__inca_native__;
    globalThis.AbortSignal = Real;
    delete (globalThis as { __inca_native__?: unknown }).__inca_native__;
    vi.doMock("./element/eventHandlers.mts", () => ({ isHandlerValue: () => true }));
    vi.doMock("./ffi/index.mts", () => ({ signalId: () => null }));
    vi.doMock("./events.mts", () => ({
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }));
    try {
      vi.resetModules();
      await import("./abortSignal.mts");
    } finally {
      vi.doUnmock("./element/eventHandlers.mts");
      vi.doUnmock("./ffi/index.mts");
      vi.doUnmock("./events.mts");
      vi.resetModules();
      globalThis.AbortSignal = stand;
      globalThis.__inca_native__ = host;
    }

    const accessors = (d?: PropertyDescriptor) => d as { get?: unknown; set?: unknown } | undefined;
    const after = accessors(Object.getOwnPropertyDescriptor(Real.prototype, "onabort"));
    expect([after?.get, after?.set]).toEqual([accessors(before)?.get, accessors(before)?.set]);
  });

  it("keeps the first copy of the accessor when the module loads again", async () => {
    const signal = new AbortSignal();
    const handler = vi.fn<() => void>();
    signal.onabort = handler;

    vi.resetModules();
    await import("./abortSignal.mts");
    const kept = signal.onabort;
    abort(signal);

    expect([kept, handler.mock.calls.length]).toEqual([handler, 1]);
  });
});

describe("signalTargetOf", () => {
  it("gives a signal as it is and nothing for any other value", () => {
    const signal = new AbortSignal();

    expect([
      signalTargetOf(signal),
      signalTargetOf({}),
      signalTargetOf(Object.create(AbortSignal.prototype) as object),
      signalTargetOf(5),
    ]).toEqual([signal, undefined, undefined, undefined]);
  });
});

describe("a once listener on a signal", () => {
  it("removes a once listener through the signal that currentTarget gives", () => {
    const signal = new AbortSignal();
    const listener = vi.fn<() => void>();

    signal.addEventListener("abort", listener, { once: true });
    abort(signal);

    expect(listener).toHaveBeenCalledTimes(1);
    expect(native.removeEventListener).toHaveBeenCalledWith(signal, "abort", 1);
  });
});

describe("the collection of a signal", () => {
  it("frees a signal with a once listener and an onabort that nothing refers to", async () => {
    let next = 0;
    globalThis.__inca_native__ = nativeStub({
      addEventListener: () => ++next,
      removeEventListener: () => true,
    } as never);
    let ref: WeakRef<object>;
    (() => {
      const signal = new AbortSignal();
      signal.addEventListener("abort", () => undefined, { once: true });
      signal.onabort = () => undefined;
      ref = new WeakRef(signal);
    })();

    await collectGarbage(() => ref.deref() === undefined);

    expect(ref.deref()).toBeUndefined();
  });
});
