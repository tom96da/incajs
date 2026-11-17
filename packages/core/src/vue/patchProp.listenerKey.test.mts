// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";
import type { MockInstance } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import * as rendererCore from "../rendererCore.mts";
import { createNodeOps } from "./nodeOps.mts";
import { createPatchProp } from "./patchProp.mts";
import type { IncaCore, IncaElement, ListenerOptions } from "../rendererCore.mts";

/** The function a registration call received, callable with any event. */
const listenerOf = (call: readonly unknown[]): ((...args: unknown[]) => void) =>
  call[2] as (...args: unknown[]) => void;

const core: Pick<
  IncaCore,
  | "addEventListener"
  | "removeEventListener"
  | "latestEventId"
  | "isRemoved"
  | "setAttribute"
  | "removeAttribute"
> = {
  addEventListener: vi.fn<IncaCore["addEventListener"]>(),
  removeEventListener: vi.fn<IncaCore["removeEventListener"]>(),
  latestEventId: vi.fn<() => number>(() => 0),
  isRemoved: () => false,
  setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  removeAttribute: vi.fn<(nodeId: number, key: string) => void>(),
};

const patchProp = createPatchProp(core as unknown as IncaCore);

const BUBBLE = { capture: false, passive: false };

/** An element holds the id of its node, which is all that patchProp reads. */
const makeEl = (): IncaElement => ({ __node_id: 1 }) as unknown as IncaElement;

let el = makeEl();

beforeEach(() => {
  vi.clearAllMocks();
  el = makeEl();
});

describe("on*", () => {
  it("registers an on* prop's camelCase name as the hyphenated event name", () => {
    patchProp(el, "onMyEvent", null, vi.fn(), undefined, null);

    expect(core.addEventListener).toHaveBeenCalledWith(1, "my-event", expect.any(Function), BUBBLE);
  });

  it("splits every capital letter of an on* prop name, as Vue does", () => {
    patchProp(el, "onXMLEvent", null, vi.fn(), undefined, null);

    expect(core.addEventListener).toHaveBeenCalledWith(
      1,
      "x-m-l-event",
      expect.any(Function),
      BUBBLE,
    );
  });

  it("strips a suffix off an on:name prop", () => {
    patchProp(el, "on:myEventOnce", null, vi.fn(), undefined, null);

    expect(core.addEventListener).toHaveBeenCalledWith(1, "myEvent", expect.any(Function), BUBBLE);
  });

  it("applies a Once suffix of an on:name prop to the listener", () => {
    const listener = vi.fn<() => void>();
    patchProp(el, "on:myEventOnce", null, listener, undefined, null);
    const registered = listenerOf(vi.mocked(core.addEventListener).mock.calls[0]!);

    registered();
    registered();
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it("passes the event arguments to the listener", () => {
    const listener = vi.fn<() => void>();
    patchProp(el, "onClick", null, listener, undefined, null);
    listenerOf(vi.mocked(core.addEventListener).mock.calls[0]!)("arg");

    expect(listener).toHaveBeenCalledWith("arg");
  });

  it("collapses an array of handlers into one listener", () => {
    const first = vi.fn<() => void>();
    const second = vi.fn<() => void>();

    patchProp(el, "onClick", null, [first, second], undefined, null);

    expect(core.addEventListener).toHaveBeenCalledTimes(1);
    const registered = listenerOf(vi.mocked(core.addEventListener).mock.calls[0]!);
    registered();
    expect(first).toHaveBeenCalledTimes(1);
    expect(second).toHaveBeenCalledTimes(1);
  });

  it("unbinds the event when the value turns undefined", () => {
    patchProp(el, "onClick", null, vi.fn(), undefined, null);
    patchProp(el, "onClick", vi.fn(), undefined, undefined, null);

    expect(core.addEventListener).toHaveBeenCalledTimes(1);
    const added = vi.mocked(core.addEventListener).mock.calls[0]![2];
    expect(core.removeEventListener).toHaveBeenCalledExactlyOnceWith(1, "click", added, BUBBLE);
  });

  it("does nothing when unbinding an event that was never bound", () => {
    patchProp(el, "onClick", vi.fn(), undefined, undefined, null);

    expect(core.addEventListener).not.toHaveBeenCalled();
    expect(core.removeEventListener).not.toHaveBeenCalled();
  });
});

describe("onUpdate:* props", () => {
  it("binds no listener for a v-model update handler", () => {
    patchProp(el, "onUpdate:modelValue", null, vi.fn(), undefined, null);

    expect(core.addEventListener).not.toHaveBeenCalled();
    expect(core.setAttribute).not.toHaveBeenCalled();
  });

  it("does nothing when the handler is removed", () => {
    patchProp(el, "onUpdate:modelValue", vi.fn(), undefined, undefined, null);

    expect(core.removeEventListener).not.toHaveBeenCalled();
    expect(core.removeAttribute).not.toHaveBeenCalled();
  });

  it("still binds an on* prop that merely starts with Update", () => {
    patchProp(el, "onUpdateRequested", null, vi.fn(), undefined, null);

    expect(core.addEventListener).toHaveBeenCalledWith(
      1,
      "update-requested",
      expect.any(Function),
      BUBBLE,
    );
  });
});

describe("on* modifiers", () => {
  it.each<[string, ListenerOptions]>([
    ["onClickCapture", { capture: true, passive: false }],
    ["onClickPassive", { capture: false, passive: true }],
    ["onClickCapturePassive", { capture: true, passive: true }],
  ])(
    "registers each suffix combination with its capture and passive flags (%s)",
    (key, options) => {
      patchProp(el, key, null, vi.fn(), undefined, null);

      expect(core.addEventListener).toHaveBeenCalledTimes(1);
      expect(vi.mocked(core.addEventListener).mock.calls[0]![1]).toBe("click");
      expect(vi.mocked(core.addEventListener).mock.calls[0]![3]).toEqual(options);
    },
  );

  it.each([
    ["onOnce", "once"],
    ["onPassive", "passive"],
    ["onCapture", "capture"],
    ["onOnceCapture", "once"],
    ["on:Once", "Once"],
    ["on:Passive", "Passive"],
    ["on:Capture", "Capture"],
  ] as const)(
    "binds a prop named after a bare modifier as the event of that name (%s)",
    (key, event) => {
      patchProp(el, key, null, vi.fn(), undefined, null);

      expect(core.addEventListener).toHaveBeenCalledTimes(1);
      expect(vi.mocked(core.addEventListener).mock.calls[0]![1]).toBe(event);
      expect(vi.mocked(core.addEventListener).mock.calls[0]![3]?.capture).toBe(
        key === "onOnceCapture",
      );
    },
  );

  it("keeps a plain, a capture and a passive listener of one event apart", () => {
    patchProp(el, "onClick", null, vi.fn(), undefined, null);
    patchProp(el, "onClickCapture", null, vi.fn(), undefined, null);
    patchProp(el, "onClickPassive", null, vi.fn(), undefined, null);
    vi.mocked(core.removeEventListener).mockClear();

    patchProp(el, "onClickCapture", vi.fn(), null, undefined, null);

    expect(core.removeEventListener).toHaveBeenCalledExactlyOnceWith(
      1,
      "click",
      expect.any(Function),
      {
        capture: true,
        passive: false,
      },
    );
  });

  it("shares one registration between slots of the same phase and passivity", () => {
    patchProp(el, "onClickCapture", null, vi.fn(), undefined, null);
    patchProp(el, "onClickOnceCapture", null, vi.fn(), undefined, null);

    expect(core.addEventListener).toHaveBeenCalledTimes(1);
  });

  it("retires a once capture listener from the host after it fires", () => {
    const listener = vi.fn<() => void>();
    patchProp(el, "onClickOnceCapture", null, listener, undefined, null);

    listenerOf(vi.mocked(core.addEventListener).mock.calls[0]!)();

    expect(listener).toHaveBeenCalledTimes(1);
    expect(core.removeEventListener).toHaveBeenCalledExactlyOnceWith(
      1,
      "click",
      expect.any(Function),
      {
        capture: true,
        passive: false,
      },
    );
  });
});

describe("onXxx values", () => {
  const core: IncaCore = {
    ...rendererCore,
    addEventListener: vi.fn<IncaCore["addEventListener"]>(),
    removeEventListener: vi.fn<IncaCore["removeEventListener"]>(),
    latestEventId: vi.fn<() => number>(() => 0),
    isRemoved: () => false,
    setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
    removeAttribute: vi.fn<(nodeId: number, key: string) => void>(),
  };

  const patchProp = createPatchProp(core);

  let el = { __node_id: 1 } as unknown as IncaElement;

  /** The listener the newest registration call received. */
  const registered = (): ((...args: unknown[]) => void) =>
    vi.mocked(core.addEventListener).mock.calls.at(-1)![2] as (...args: unknown[]) => void;

  beforeEach(() => {
    vi.clearAllMocks();
    el = { __node_id: 1 } as unknown as IncaElement;
  });

  let warn: MockInstance<typeof console.warn>;

  beforeEach(() => {
    warn = vi.spyOn(console, "warn").mockImplementation(() => {});
  });
  afterEach(() => {
    warn.mockRestore();
  });

  it("warns about a value that is not a function", () => {
    patchProp(el, "onClick", null, "count++", undefined, null);

    expect(warn).toHaveBeenCalledOnce();
    expect(warn.mock.calls[0]![0]).toEqual(
      expect.stringContaining(
        "Wrong type passed as event handler to onClick - did you forget @ or : in front of your prop?\nExpected function or array of functions, received type string.",
      ),
    );
  });

  it("keeps a no-op listener when a function turns into a string", () => {
    const fn = vi.fn<() => void>();
    patchProp(el, "onClick", null, fn, undefined, null);
    patchProp(el, "onClick", fn, "count++", undefined, null);
    registered()();

    expect(core.removeEventListener).not.toHaveBeenCalled();
    expect(fn).not.toHaveBeenCalled();
  });

  it("keeps the listener when the value turns into an empty array", () => {
    patchProp(el, "onClick", null, vi.fn(), undefined, null);
    patchProp(el, "onClick", vi.fn(), [], undefined, null);

    expect(core.removeEventListener).not.toHaveBeenCalled();
    expect(warn).not.toHaveBeenCalled();
  });

  it("unbinds the event when the value turns into an empty string", () => {
    patchProp(el, "onClick", null, vi.fn(), undefined, null);
    patchProp(el, "onClick", vi.fn(), "", undefined, null);

    expect(core.removeEventListener).toHaveBeenCalledOnce();
  });

  it("skips a null array entry without a warning", () => {
    const handler = vi.fn<() => void>();
    patchProp(el, "onClick", null, [null, handler], undefined, null);
    registered()();

    expect(handler).toHaveBeenCalledOnce();
    expect(warn).not.toHaveBeenCalled();
  });

  it("runs the functions of a nested array", () => {
    const handler = vi.fn<() => void>();
    patchProp(el, "onClick", null, [[handler]], undefined, null);
    registered()();

    expect(handler).toHaveBeenCalledOnce();
  });

  it("warns at dispatch about an array entry that is not a function", () => {
    patchProp(el, "onClick", null, ["x", vi.fn<() => void>()], undefined, null);
    registered()();

    expect(warn).toHaveBeenCalledOnce();
  });

  it("runs the entries after an array entry that is not a function", () => {
    const handler = vi.fn<() => void>();
    patchProp(el, "onClick", null, ["x", handler], undefined, null);
    registered()();

    expect(handler).toHaveBeenCalledOnce();
  });
});

describe("patchProp on a removed element", () => {
  const core = rendererCore;

  it("adds a listener slot without reaching the host", () => {
    const released = new Set<number>();
    vi.stubGlobal(
      "__inca_native__",
      nativeStub({
        createNode: vi.fn<() => number>(() => 2),
        holdNode: vi.fn<() => void>(),
        releaseNode: vi.fn<() => void>(),
        removeNode: vi.fn<(id: number) => [number[], number[]]>((id) => {
          released.add(id);
          return [[], [id]];
        }),
        addEventListener: vi.fn<(id: number) => number>((id) => {
          if (released.has(id)) throw new TypeError(`node ${id} is not live`);
          return 0;
        }),
        removeEventListener: vi.fn<() => boolean>(() => true),
      } as never),
    );
    const ops = createNodeOps(core);
    const el = ops.createElement("view") as IncaElement;
    ops.remove(el);

    const patch = createPatchProp(core);

    expect(() => patch(el, "onClick", null, () => {}, undefined, null)).not.toThrow();
  });
});
