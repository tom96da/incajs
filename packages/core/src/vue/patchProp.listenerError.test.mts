// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { createRenderer, h, onErrorCaptured } from "@vue/runtime-core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ComponentInternalInstance } from "@vue/runtime-core";
import type { MockInstance } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import * as rendererCore from "../rendererCore.mts";
import { createNodeOps } from "./nodeOps.mts";
import { createPatchProp } from "./patchProp.mts";
import type { IncaCore, IncaElement } from "../rendererCore.mts";

/** The function a registration call received, callable with any event. */
const listenerOf = (call: readonly unknown[]): ((...args: unknown[]) => void) =>
  call[2] as (...args: unknown[]) => void;

const core: IncaCore = {
  ...rendererCore,
  addEventListener: vi.fn<IncaCore["addEventListener"]>(),
  removeEventListener: vi.fn<IncaCore["removeEventListener"]>(),
  latestEventId: vi.fn<() => number>(() => 0),
  isRemoved: () => false,
};

const patchProp = createPatchProp(core);

const BUBBLE = { capture: false, passive: false };

/** An element holds the id of its node, which is all that patchProp reads. */
const makeEl = (): IncaElement => ({ __node_id: 1 }) as unknown as IncaElement;

let el = makeEl();

beforeEach(() => {
  vi.clearAllMocks();
  el = makeEl();
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

  let warn: MockInstance<typeof console.warn>;
  beforeEach(() => {
    warn = vi.spyOn(console, "warn").mockImplementation(() => {});
  });
  afterEach(() => {
    warn.mockRestore();
  });

  it("still propagates when no errorHandler is configured", () => {
    patchProp(el, "onClick", null, throwing, undefined, instanceWith());

    const registered = listenerOf(vi.mocked(core.addEventListener).mock.calls[0]!);
    expect(() => registered()).toThrow(boom);
    expect(warn).toHaveBeenCalledWith(expect.stringContaining("Unhandled error"));
  });

  describe("onErrorCaptured hooks", () => {
    const withHook = (hook: () => boolean | void): ComponentInternalInstance => {
      const child = instanceWith();
      (child as unknown as { parent: unknown }).parent = { ec: [hook], parent: null };
      return child;
    };

    it("receives the error up the parent chain and a false return suppresses the report", () => {
      const hook = vi.fn<() => boolean>(() => false);
      patchProp(el, "onClick", null, throwing, undefined, withHook(hook));

      const registered = listenerOf(vi.mocked(core.addEventListener).mock.calls[0]!);
      expect(() => registered()).not.toThrow();
      expect(hook).toHaveBeenCalledWith(boom, undefined, "native event handler");
      expect(warn).not.toHaveBeenCalled();
    });

    it("reports to the host when the hook does not return false", () => {
      patchProp(
        el,
        "onClick",
        null,
        throwing,
        undefined,
        withHook(() => {}),
      );

      const registered = listenerOf(vi.mocked(core.addEventListener).mock.calls[0]!);
      expect(() => registered()).toThrow(boom);
    });
  });

  describe("several handlers", () => {
    const second = new Error("second");
    const third = new Error("third");
    const secondThrowing = (): void => {
      throw second;
    };
    const thirdThrowing = (): void => {
      throw third;
    };
    const unhandled = vi.fn<(reason: unknown) => void>();
    beforeEach(() => {
      unhandled.mockClear();
      process.on("unhandledRejection", unhandled);
    });
    afterEach(() => {
      process.off("unhandledRejection", unhandled);
    });
    const reasons = (): unknown[] => unhandled.mock.calls.map(([reason]) => reason);
    const settle = (): Promise<unknown> => new Promise((resolve) => setTimeout(resolve, 0));
    const registerWith = (
      value: unknown,
      instance = instanceWith(),
      key = "onClick",
    ): ((...args: unknown[]) => void) => {
      patchProp(el, key, null, value, undefined, instance);
      return listenerOf(vi.mocked(core.addEventListener).mock.calls.at(-1)!);
    };
    const stopper = (log: string[], name: string) => (e: unknown) => {
      log.push(name);
      (e as { stopImmediatePropagation(): void }).stopImmediatePropagation();
    };

    it("runs every handler and reports each failure once, in order", async () => {
      const after = vi.fn<() => void>();
      const registered = registerWith([throwing, secondThrowing, thirdThrowing, after]);

      expect(() => registered()).toThrow(boom);
      await settle();
      expect(after).toHaveBeenCalledTimes(1);
      expect(reasons()).toEqual([second, third]);
      expect(warn).toHaveBeenCalledTimes(3);
    });

    it("reports nothing extra when only one handler fails", async () => {
      const registered = registerWith([vi.fn<() => void>(), throwing, vi.fn<() => void>()]);

      expect(() => registered()).toThrow(boom);
      await settle();
      expect(unhandled).not.toHaveBeenCalled();
      expect(warn).toHaveBeenCalledTimes(1);
    });

    it("throws nothing when no handler fails", async () => {
      const registered = registerWith([vi.fn<() => void>(), vi.fn<() => void>()]);

      expect(() => registered()).not.toThrow();
      await settle();
      expect(unhandled).not.toHaveBeenCalled();
      expect(warn).not.toHaveBeenCalled();
    });

    it("reports failures in slot order across an array slot and a later slot", async () => {
      patchProp(el, "onClick", null, [throwing, secondThrowing], undefined, instanceWith());
      patchProp(el, "onClickOnce", null, thirdThrowing, undefined, instanceWith());
      const registered = listenerOf(vi.mocked(core.addEventListener).mock.calls[0]!);

      expect(() => registered()).toThrow(boom);
      await settle();
      expect(reasons()).toEqual([second, third]);
      expect(warn).toHaveBeenCalledTimes(3);
    });

    it("gives every failure to the errorHandler and reports none to the host", async () => {
      const errorHandler = vi.fn<(error: unknown) => void>();
      const registered = registerWith([throwing, secondThrowing], instanceWith(errorHandler));

      expect(() => registered()).not.toThrow();
      await settle();
      expect(errorHandler.mock.calls.map(([e]) => e)).toEqual([boom, second]);
      expect(unhandled).not.toHaveBeenCalled();
      expect(warn).not.toHaveBeenCalled();
    });

    it("reports each rejection of several async handlers once", async () => {
      const registered = registerWith([
        () => Promise.reject(boom),
        () => Promise.reject(second),
        () => Promise.resolve(),
      ]);

      expect(() => registered()).not.toThrow();
      await settle();
      expect(reasons()).toEqual([boom, second]);
    });

    it("reports a synchronous failure and an async rejection of one dispatch", async () => {
      const registered = registerWith([throwing, () => Promise.reject(second)]);

      expect(() => registered()).toThrow(boom);
      await settle();
      expect(reasons()).toEqual([second]);
    });

    it("retires a .once slot whose array fails and reports each failure", async () => {
      const registered = registerWith([throwing, secondThrowing], instanceWith(), "onClickOnce");

      expect(() => registered()).toThrow(boom);
      await settle();
      expect(reasons()).toEqual([second]);
      expect(core.removeEventListener).toHaveBeenCalledWith(
        1,
        "click",
        expect.any(Function),
        BUBBLE,
      );
    });

    it("stops the rest of an array and later slots at stopImmediatePropagation", async () => {
      const log: string[] = [];
      const event = { stopImmediatePropagation: vi.fn<() => void>() };
      patchProp(
        el,
        "onClick",
        null,
        [() => log.push("a"), stopper(log, "stop"), () => log.push("b")],
        undefined,
        instanceWith(),
      );
      patchProp(el, "onClickOnce", null, () => log.push("later"), undefined, instanceWith());
      const registered = listenerOf(vi.mocked(core.addEventListener).mock.calls[0]!);

      expect(() => registered(event)).not.toThrow();
      expect(log).toEqual(["a", "stop"]);
      expect(event.stopImmediatePropagation).toHaveBeenCalledTimes(1);
    });

    it("reports a failure that precedes stopImmediatePropagation and skips the rest", async () => {
      const log: string[] = [];
      const event = { stopImmediatePropagation: vi.fn<() => void>() };
      const registered = registerWith([throwing, stopper(log, "stop"), secondThrowing]);

      expect(() => registered(event)).toThrow(boom);
      await settle();
      expect(log).toEqual(["stop"]);
      expect(unhandled).not.toHaveBeenCalled();
    });
  });

  describe("in a mounted Vue app", () => {
    const unhandled = vi.fn<(reason: unknown) => void>();
    beforeEach(() => {
      unhandled.mockClear();
      process.on("unhandledRejection", unhandled);
    });
    afterEach(() => {
      process.off("unhandledRejection", unhandled);
    });

    // Mounts a tree whose `view` carries `onClick`, under a parent with `hook`.
    function mount(
      onClick: unknown[],
      configure: (app: { config: { errorHandler?: unknown } }) => void,
      hook?: () => boolean | void,
    ): () => void {
      let id = 1;
      vi.stubGlobal(
        "__inca_native__",
        nativeStub({
          createNode: () => ++id,
          insertBefore: () => {},
          linkedNode: () => null,
        } as never),
      );
      const renderer = createRenderer({ ...createNodeOps(core), patchProp });
      const Child = { render: () => h("view", { onClick }) };
      const Parent = {
        setup() {
          if (hook) onErrorCaptured(hook);
          return () => h(Child);
        },
      };
      const app = renderer.createApp(Parent);
      configure(app);
      app.mount(el);
      return listenerOf(vi.mocked(core.addEventListener).mock.calls.at(-1)!);
    }

    const first = new Error("first");
    const second = new Error("second");
    const fail = (error: Error) => () => {
      throw error;
    };

    it("gives each failure to onErrorCaptured and then the errorHandler", () => {
      const errorHandler = vi.fn<(error: unknown) => void>();
      const hook = vi.fn<() => void>();
      const dispatch = mount(
        [fail(first), fail(second)],
        (a) => (a.config.errorHandler = errorHandler),
        hook,
      );

      expect(() => dispatch()).not.toThrow();
      expect(hook).toHaveBeenCalledTimes(2);
      expect(errorHandler.mock.calls.map(([e]) => e)).toEqual([first, second]);
    });
  });

  describe("async rejections", () => {
    const unhandled = vi.fn<(reason: unknown, promise: unknown) => void>();
    beforeEach(() => {
      unhandled.mockClear();
      process.on("unhandledRejection", unhandled);
    });
    afterEach(() => {
      process.off("unhandledRejection", unhandled);
    });

    async function register(handler: () => unknown, instance: ComponentInternalInstance | null) {
      patchProp(el, "onClick", null, handler, undefined, instance);
      const registered = listenerOf(vi.mocked(core.addEventListener).mock.calls[0]!);
      expect(() => registered()).not.toThrow();
      await new Promise((resolve) => setTimeout(resolve, 0));
    }

    it("hands the rejection to the errorHandler and reports nothing else", async () => {
      const errorHandler = vi.fn<() => void>();
      await register(() => Promise.reject(boom), instanceWith(errorHandler));
      expect(errorHandler).toHaveBeenCalledWith(boom, undefined, "native event handler");
      expect(unhandled).not.toHaveBeenCalled();
    });

    it("reports nothing when the promise resolves", async () => {
      await register(() => Promise.resolve(), instanceWith());
      expect(unhandled).not.toHaveBeenCalled();
    });
  });

  it("still propagates with no owning component", () => {
    patchProp(el, "onClick", null, throwing, undefined, null);

    const registered = listenerOf(vi.mocked(core.addEventListener).mock.calls[0]!);
    expect(() => registered()).toThrow(boom);
  });
});

describe("with a production build of Vue", () => {
  const boom = new Error("boom");
  const unhandled = vi.fn<(reason: unknown) => void>();
  let error: MockInstance<typeof console.error>;
  let warn: MockInstance<typeof console.warn>;
  let dispatchOf: (value: unknown, config?: Record<string, unknown>) => () => void;

  beforeEach(async () => {
    vi.resetModules();
    const prodBuild = "@vue/runtime-core/dist/runtime-core.cjs.prod.js";
    vi.doMock("@vue/runtime-core", () => import(/* @vite-ignore */ prodBuild));
    const { createPatchProp: createProd } = await import("./patchProp.mts");
    const prodPatchProp = createProd(core);
    dispatchOf = (value, config = {}) => {
      const instance = { vnode: null, parent: null, appContext: { config } };
      prodPatchProp(el, "onClick", null, value, undefined, instance as never);
      return listenerOf(vi.mocked(core.addEventListener).mock.calls.at(-1)!);
    };
    error = vi.spyOn(console, "error").mockImplementation(() => {});
    warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    unhandled.mockClear();
    process.on("unhandledRejection", unhandled);
  });
  afterEach(() => {
    process.off("unhandledRejection", unhandled);
    error.mockRestore();
    warn.mockRestore();
    vi.doUnmock("@vue/runtime-core");
    vi.resetModules();
  });
  const settle = (): Promise<unknown> => new Promise((resolve) => setTimeout(resolve, 0));

  it("logs a synchronous failure to the console and throws nothing", () => {
    const dispatch = dispatchOf(() => {
      throw boom;
    });

    expect(() => dispatch()).not.toThrow();
    expect(error).toHaveBeenCalledTimes(1);
    expect(error).toHaveBeenCalledWith(boom);
    expect(warn).not.toHaveBeenCalled();
  });

  it("logs each failing handler of an array once and runs them all", () => {
    const after = vi.fn<() => void>();
    const dispatch = dispatchOf([
      () => {
        throw boom;
      },
      after,
    ]);

    expect(() => dispatch()).not.toThrow();
    expect(error).toHaveBeenCalledTimes(1);
    expect(after).toHaveBeenCalledTimes(1);
  });

  it("logs an async rejection once and reports no unhandled rejection", async () => {
    const dispatch = dispatchOf(() => Promise.reject(boom));

    expect(() => dispatch()).not.toThrow();
    await settle();
    expect(error).toHaveBeenCalledTimes(1);
    expect(unhandled).not.toHaveBeenCalled();
  });

  it("throws to the host with throwUnhandledErrorInProduction", () => {
    const dispatch = dispatchOf(
      () => {
        throw boom;
      },
      { throwUnhandledErrorInProduction: true },
    );

    expect(() => dispatch()).toThrow(boom);
    expect(error).not.toHaveBeenCalled();
  });

  it("hands the failure to the errorHandler and logs nothing", () => {
    const errorHandler = vi.fn<() => void>();
    const dispatch = dispatchOf(
      () => {
        throw boom;
      },
      { errorHandler },
    );

    expect(() => dispatch()).not.toThrow();
    expect(errorHandler).toHaveBeenCalledTimes(1);
    expect(error).not.toHaveBeenCalled();
  });
});

describe("with a production build of Vue", () => {
  const core: IncaCore = {
    ...rendererCore,
    addEventListener: vi.fn<IncaCore["addEventListener"]>(),
    removeEventListener: vi.fn<IncaCore["removeEventListener"]>(),
    latestEventId: vi.fn<() => number>(() => 0),
    isRemoved: () => false,
    setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
    removeAttribute: vi.fn<(nodeId: number, key: string) => void>(),
  };

  let el = { __node_id: 1 } as unknown as IncaElement;

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
    vi.doUnmock("@vue/runtime-core");
    vi.resetModules();
  });

  it("keeps a silent no-op listener for a value that is not a function", async () => {
    vi.resetModules();
    const prodBuild = "@vue/runtime-core/dist/runtime-core.cjs.prod.js";
    vi.doMock("@vue/runtime-core", () => import(/* @vite-ignore */ prodBuild));
    const { createPatchProp: createProd } = await import("./patchProp.mts");

    createProd(core)(el, "onClick", null, "count++", undefined, null);

    expect(core.addEventListener).toHaveBeenCalledOnce();
    expect(warn).not.toHaveBeenCalled();
  });
});
