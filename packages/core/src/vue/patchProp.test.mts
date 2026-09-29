// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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
  removeStyle: vi.fn<(nodeId: number, key: string) => void>(),
  focus: vi.fn<(nodeId: number) => void>(),
  blur: vi.fn<(nodeId: number) => void>(),
};

const patchProp = createPatchProp(core);

const makeEl = (): IncaElement => ({
  id: 1,
  kind: "element",
  parent: null,
  children: [],
  focus: vi.fn<() => void>(),
  blur: vi.fn<() => void>(),
});

let el = makeEl();

beforeEach(() => {
  vi.clearAllMocks();
  el = makeEl();
});

// Mounts `prev` as the element's style, then forgets those calls.
function mountStyle(prev: unknown): void {
  patchProp(el, "style", null, prev, undefined, null);
  vi.clearAllMocks();
}

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

  it("removes every key when the whole style goes away", () => {
    mountStyle({ background: "#000", gap: 8 });
    patchProp(el, "style", { background: "#000", gap: 8 }, null, undefined, null);

    expect(core.setStyle).not.toHaveBeenCalled();
    expect(core.removeStyle).toHaveBeenCalledWith(1, "background");
    expect(core.removeStyle).toHaveBeenCalledWith(1, "gap");
    expect(core.removeStyle).toHaveBeenCalledTimes(2);
  });

  it("removes a key that disappears or turns null or undefined", () => {
    mountStyle({ background: "#000", gap: 8, width: 1 });
    patchProp(
      el,
      "style",
      { background: "#000", gap: 8, width: 1 },
      { gap: null, width: undefined },
      undefined,
      null,
    );

    expect(core.removeStyle).toHaveBeenCalledWith(1, "background");
    expect(core.removeStyle).toHaveBeenCalledWith(1, "gap");
    expect(core.removeStyle).toHaveBeenCalledWith(1, "width");
    expect(core.setStyle).not.toHaveBeenCalled();
  });

  it("re-sends only changed keys", () => {
    mountStyle({ background: "#000", gap: 8 });
    patchProp(
      el,
      "style",
      { background: "#000", gap: 8 },
      { background: "#000", gap: 9 },
      undefined,
      null,
    );

    expect(core.setStyle).toHaveBeenCalledExactlyOnceWith(1, "gap", 9);
    expect(core.removeStyle).not.toHaveBeenCalled();
  });

  it("removes every sent key when the style becomes a string", () => {
    const other = { ...el, id: 2 };
    patchProp(other, "style", null, { gap: 8 }, undefined, null);
    patchProp(other, "style", { gap: 8 }, "color: red", undefined, null);

    expect(core.setStyle).toHaveBeenCalledExactlyOnceWith(2, "gap", 8);
    expect(core.removeStyle).toHaveBeenCalledExactlyOnceWith(2, "gap");
  });

  it("applies an in-place change when prev and next are the same object", () => {
    const other = { ...el, id: 3 };
    const style: Record<string, unknown> = { gap: 8, width: 1 };
    patchProp(other, "style", null, style, undefined, null);
    vi.clearAllMocks();

    style.gap = 9;
    patchProp(other, "style", style, style, undefined, null);

    expect(core.setStyle).toHaveBeenCalledExactlyOnceWith(3, "gap", 9);
    expect(core.removeStyle).not.toHaveBeenCalled();
  });

  it("removes a key deleted in place", () => {
    const other = { ...el, id: 4 };
    const style: Record<string, unknown> = { gap: 8, width: 1 };
    patchProp(other, "style", null, style, undefined, null);
    vi.clearAllMocks();

    delete style.gap;
    patchProp(other, "style", style, style, undefined, null);

    expect(core.removeStyle).toHaveBeenCalledExactlyOnceWith(4, "gap");
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

  describe("async rejections", () => {
    const unhandled = vi.fn<(reason: unknown, promise: unknown) => void>();
    beforeEach(() => {
      unhandled.mockClear();
      process.on("unhandledRejection", unhandled);
    });
    afterEach(() => {
      process.off("unhandledRejection", unhandled);
      vi.unstubAllEnvs();
      vi.unstubAllGlobals();
      vi.resetModules();
    });

    // `isProduction` is fixed when the module loads, so each case reloads it.
    async function register(mode: string, instance: ComponentInternalInstance | null) {
      vi.stubEnv("NODE_ENV", mode);
      vi.resetModules();
      const { createPatchProp: fresh } = await import("./patchProp.mts");
      fresh(core)!(el, "onClick", null, () => Promise.reject(boom), undefined, instance);
      const registered = vi.mocked(core.setEventListener).mock.calls[0]![2];
      registered();
      await new Promise((resolve) => setTimeout(resolve, 0));
    }

    // The Vue under test is always its dev build, which rethrows an async
    // failure itself, so a production bundle shows up as one extra report.
    it.each([
      ["development", 1],
      ["production", 2],
    ])("with NODE_ENV=%s and no errorHandler reports %i time(s)", async (mode, count) => {
      await register(mode, instanceWith());
      expect(unhandled).toHaveBeenCalledTimes(count);
    });

    it("hands the rejection to the errorHandler and adds no report, in production", async () => {
      const errorHandler = vi.fn<() => void>();
      await register("production", instanceWith(errorHandler));
      expect(errorHandler).toHaveBeenCalledWith(boom, undefined, "native event handler");
      expect(unhandled).not.toHaveBeenCalled();
    });

    it("still runs the handler when reading `process.env` throws", async () => {
      const broken = Object.create(process, {
        env: {
          get() {
            throw new ReferenceError("process is not defined");
          },
        },
      });
      vi.stubGlobal("process", broken);
      vi.resetModules();
      const { createPatchProp: fresh } = await import("./patchProp.mts");
      const handler = vi.fn<() => void>();
      fresh(core)!(el, "onClick", null, handler, undefined, null);
      vi.unstubAllGlobals();
      vi.mocked(core.setEventListener).mock.calls[0]![2]();
      expect(handler).toHaveBeenCalledTimes(1);
    });
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
