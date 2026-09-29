// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ComponentInternalInstance } from "@vue/runtime-core";

import { createPatchProp } from "./patchProp.mts";
import type { IncaCore } from "../rendererCore.mts";
import type { IncaElement } from "./nodeOps.mts";

const core: IncaCore = {
  rootNodeId: vi.fn<() => number>(),
  createNode: vi.fn<(tag: string) => number>(),
  appendChild: vi.fn<(parentId: number, childId: number) => void>(),
  insertBefore: vi.fn<(parentId: number, childId: number, anchorId: number | null) => void>(),
  removeChild: vi.fn<(parentId: number, childId: number) => void>(),
  destroyNode: vi.fn<(nodeId: number) => void>(),
  setEventListener:
    vi.fn<(nodeId: number, event: string, listener: (...args: unknown[]) => void) => void>(),
  removeEventListener: vi.fn<(nodeId: number, event: string) => void>(),
  setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  setStyle: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  focus: vi.fn<(nodeId: number) => void>(),
  blur: vi.fn<(nodeId: number) => void>(),
};

const patchProp = createPatchProp(core);

const el: IncaElement = {
  id: 1,
  kind: "element",
  parent: null,
  children: [],
  focus: vi.fn<() => void>(),
  blur: vi.fn<() => void>(),
};

beforeEach(() => {
  vi.clearAllMocks();
});

describe("style", () => {
  it("calls setStyle once per primitive-valued entry", () => {
    patchProp(el, "style", null, { background: "#000", gap: 8 }, undefined, null);

    expect(core.setStyle).toHaveBeenCalledWith(1, "background", "#000");
    expect(core.setStyle).toHaveBeenCalledWith(1, "gap", 8);
    expect(core.setStyle).toHaveBeenCalledTimes(2);
  });

  it("skips entries whose value isn't a primitive setStyle can take", () => {
    patchProp(el, "style", null, { border_width: [1, 2] }, undefined, null);

    expect(core.setStyle).not.toHaveBeenCalled();
  });

  it("does nothing for a null style value", () => {
    patchProp(el, "style", { background: "#000" }, null, undefined, null);

    expect(core.setStyle).not.toHaveBeenCalled();
  });
});

describe("on*", () => {
  it("registers a function listener under the lower-cased event name", () => {
    const listener = vi.fn<() => void>();
    patchProp(el, "onClick", null, listener, undefined, null);

    expect(core.setEventListener).toHaveBeenCalledWith(1, "click", expect.any(Function));
    vi.mocked(core.setEventListener).mock.calls[0]![2]("arg");
    expect(listener).toHaveBeenCalledWith("arg");
  });

  it("collapses an array of handlers into one listener", () => {
    const first = vi.fn<() => void>();
    const second = vi.fn<() => void>();

    patchProp(el, "onClick", null, [first, second], undefined, null);

    expect(core.setEventListener).toHaveBeenCalledTimes(1);
    const registered = vi.mocked(core.setEventListener).mock.calls[0]![2];
    registered();
    expect(first).toHaveBeenCalledTimes(1);
    expect(second).toHaveBeenCalledTimes(1);
  });

  it("unbinds the event when the value is no longer a function", () => {
    patchProp(el, "onClick", vi.fn(), undefined, undefined, null);

    expect(core.setEventListener).not.toHaveBeenCalled();
    expect(core.removeEventListener).toHaveBeenCalledWith(1, "click");
  });
});

describe("on* modifiers", () => {
  it("fires an .once listener on the first dispatch, then unbinds it", () => {
    const listener = vi.fn<() => void>();
    patchProp(el, "onClickOnce", null, listener, undefined, null);

    expect(core.setEventListener).toHaveBeenCalledWith(1, "click", expect.any(Function));
    const registered = vi.mocked(core.setEventListener).mock.calls[0]![2];

    registered();
    expect(listener).toHaveBeenCalledTimes(1);
    expect(core.removeEventListener).toHaveBeenCalledWith(1, "click");
  });

  it("binds .passive as an ordinary listener that fires", () => {
    const listener = vi.fn<() => void>();
    patchProp(el, "onClickPassive", null, listener, undefined, null);

    const registered = vi.mocked(core.setEventListener).mock.calls[0]![2];
    registered();
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it("binds .capture as an ordinary listener that fires", () => {
    const listener = vi.fn<() => void>();
    patchProp(el, "onClickCapture", null, listener, undefined, null);

    const registered = vi.mocked(core.setEventListener).mock.calls[0]![2];
    registered();
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it("strips combined modifier suffixes down to the event name", () => {
    const listener = vi.fn<() => void>();
    patchProp(el, "onClickOnceCapture", null, listener, undefined, null);

    expect(core.setEventListener).toHaveBeenCalledWith(1, "click", expect.any(Function));
  });
});

describe("errors thrown by an event handler", () => {
  const boom = new Error("boom");
  const throwing = (): void => {
    throw boom;
  };
  const instanceWith = (errorHandler?: unknown): ComponentInternalInstance =>
    ({
      vnode: null,
      parent: null,
      appContext: { config: { errorHandler } },
    }) as unknown as ComponentInternalInstance;

  it("reaches the app's errorHandler and is not rethrown", () => {
    const errorHandler = vi.fn<() => void>();
    patchProp(el, "onClick", null, throwing, undefined, instanceWith(errorHandler));

    const registered = vi.mocked(core.setEventListener).mock.calls[0]![2];
    expect(() => registered()).not.toThrow();
    expect(errorHandler).toHaveBeenCalledWith(boom, undefined, "native event handler");
  });

  it("still propagates when no errorHandler is configured", () => {
    patchProp(el, "onClick", null, throwing, undefined, instanceWith());

    const registered = vi.mocked(core.setEventListener).mock.calls[0]![2];
    expect(() => registered()).toThrow(boom);
  });

  it("calls onErrorCaptured hooks up the parent chain", () => {
    const hook = vi.fn<() => boolean>(() => false);
    const child = instanceWith();
    (child as unknown as { parent: unknown }).parent = { ec: [hook], parent: null };
    patchProp(el, "onClick", null, throwing, undefined, child);

    const registered = vi.mocked(core.setEventListener).mock.calls[0]![2];
    expect(() => registered()).toThrow(boom);
    expect(hook).toHaveBeenCalledWith(boom, undefined, "native event handler");
  });

  it("rethrows the first failure of an array of handlers", () => {
    const second = new Error("second");
    patchProp(
      el,
      "onClick",
      null,
      [
        throwing,
        () => {
          throw second;
        },
      ],
      undefined,
      instanceWith(),
    );

    const registered = vi.mocked(core.setEventListener).mock.calls[0]![2];
    expect(() => registered()).toThrow(boom);
  });

  it("unbinds a .once listener and still reports its error", () => {
    patchProp(el, "onClickOnce", null, throwing, undefined, instanceWith());

    const registered = vi.mocked(core.setEventListener).mock.calls[0]![2];
    expect(() => registered()).toThrow(boom);
    expect(core.removeEventListener).toHaveBeenCalledWith(1, "click");
  });

  it("still propagates with no owning component", () => {
    patchProp(el, "onClick", null, throwing, undefined, null);

    const registered = vi.mocked(core.setEventListener).mock.calls[0]![2];
    expect(() => registered()).toThrow(boom);
  });
});

describe("everything else", () => {
  it.each([
    ["label", "hello"],
    ["count", 3],
    ["disabled", true],
  ])("forwards %s=%p to setAttribute", (key, value) => {
    patchProp(el, key, null, value, undefined, null);

    expect(core.setAttribute).toHaveBeenCalledWith(1, key, value);
  });

  it("skips a value setAttribute can't take", () => {
    patchProp(el, "data", null, { nested: true }, undefined, null);

    expect(core.setAttribute).not.toHaveBeenCalled();
  });
});
