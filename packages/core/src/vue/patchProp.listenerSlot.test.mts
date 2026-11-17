// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MockInstance } from "vitest";

import { createPatchProp } from "./patchProp.mts";
import type { IncaCore, IncaElement } from "../rendererCore.mts";

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

describe("plain and .once listeners on one element", () => {
  const set = (key: string, value: unknown, target = el): void =>
    patchProp(target, key, null, value, undefined, null);
  // Runs the most recent host registration for (id, event).
  const fire = (event = "click", id = 1): void => {
    const calls = vi.mocked(core.addEventListener).mock.calls;
    listenerOf(calls.findLast(([nodeId, name]) => nodeId === id && name === event)!)();
  };
  const fn = (log: string[], name: string) => vi.fn<() => void>(() => void log.push(name));

  let warn: MockInstance<typeof console.warn>;
  let error: MockInstance<typeof console.error>;
  beforeEach(() => {
    warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    error = vi.spyOn(console, "error").mockImplementation(() => {});
  });
  afterEach(() => {
    warn.mockRestore();
    error.mockRestore();
  });

  it("keeps a plain listener registered after it fires", () => {
    const plain = vi.fn<() => void>();
    set("onClick", plain);
    fire();
    fire();

    expect(core.addEventListener).toHaveBeenCalledTimes(1);
    expect(plain).toHaveBeenCalledTimes(2);
    expect(core.removeEventListener).not.toHaveBeenCalled();
  });

  it("unbinds the host only when the last slot goes, after a once fired", () => {
    const plain = vi.fn<() => void>();
    set("onClick", plain);
    set("onClickOnce", vi.fn());
    fire();
    expect(core.removeEventListener).not.toHaveBeenCalled();

    patchProp(el, "onClick", plain, undefined, undefined, null);
    expect(core.removeEventListener).toHaveBeenCalledExactlyOnceWith(
      1,
      "click",
      expect.any(Function),
      BUBBLE,
    );
  });

  it("re-registers with the host after every slot went away", () => {
    set("onClick", vi.fn());
    patchProp(el, "onClick", null, undefined, undefined, null);
    set("onClick", vi.fn());

    expect(core.addEventListener).toHaveBeenCalledTimes(2);
    expect(core.removeEventListener).toHaveBeenCalledTimes(1);
  });

  it("replaces a plain handler in place without touching the once slot or the host", () => {
    const log: string[] = [];
    set("onClick", fn(log, "plain-1"));
    set("onClickOnce", fn(log, "once"));
    patchProp(el, "onClick", null, fn(log, "plain-2"), undefined, null);
    fire();

    expect(core.addEventListener).toHaveBeenCalledTimes(1);
    expect(core.removeEventListener).not.toHaveBeenCalled();
    expect(log).toEqual(["plain-2", "once"]);
  });

  it("replaces a once handler in place, keeping its position and plain slot", () => {
    const log: string[] = [];
    set("onClickOnce", fn(log, "once-1"));
    set("onClick", fn(log, "plain"));
    patchProp(el, "onClickOnce", null, fn(log, "once-2"), undefined, null);
    fire();
    fire();

    expect(log).toEqual(["once-2", "plain", "plain"]);
  });

  it("removes the plain handler alone", () => {
    const log: string[] = [];
    set("onClick", fn(log, "plain"));
    set("onClickOnce", fn(log, "once"));
    patchProp(el, "onClick", null, undefined, undefined, null);
    fire();

    expect(log).toEqual(["once"]);
    expect(core.removeEventListener).toHaveBeenCalledExactlyOnceWith(
      1,
      "click",
      expect.any(Function),
      BUBBLE,
    );
  });

  it("removes the once handler alone, before it ever fires", () => {
    const log: string[] = [];
    set("onClick", fn(log, "plain"));
    set("onClickOnce", fn(log, "once"));
    patchProp(el, "onClickOnce", null, undefined, undefined, null);
    fire();
    fire();

    expect(log).toEqual(["plain", "plain"]);
    expect(core.removeEventListener).not.toHaveBeenCalled();
  });

  it("toggles from onClick to onClickOnce, moving the handler to the end", () => {
    const log: string[] = [];
    const other = fn(log, "other");
    const toggled = fn(log, "toggled");
    set("onClick", toggled);
    // A repeated suffix is a distinct raw key on the same entry.
    set("onClickOnceOnce", other);
    // A key change unpatches the old key, then patches the new one.
    patchProp(el, "onClick", toggled, undefined, undefined, null);
    patchProp(el, "onClickOnce", undefined, toggled, undefined, null);
    fire();

    expect(log).toEqual(["other", "toggled"]);
    expect(core.addEventListener).toHaveBeenCalledTimes(1);
    expect(core.removeEventListener).toHaveBeenCalledExactlyOnceWith(
      1,
      "click",
      expect.any(Function),
      BUBBLE,
    );
  });

  it("registers a distinct event on the same node separately", () => {
    const click = vi.fn<() => void>();
    const key = vi.fn<() => void>();
    set("onClick", click);
    set("onKeydown", key);
    set("onClickOnce", vi.fn());

    expect(core.addEventListener).toHaveBeenCalledTimes(2);
    fire("keydown");
    expect(key).toHaveBeenCalledTimes(1);
    expect(click).not.toHaveBeenCalled();
  });

  it("leaves another event untouched when a slot goes away", () => {
    const key = vi.fn<() => void>();
    set("onClick", vi.fn());
    set("onKeydown", key);
    patchProp(el, "onClick", null, undefined, undefined, null);

    expect(core.removeEventListener).toHaveBeenCalledExactlyOnceWith(
      1,
      "click",
      expect.any(Function),
      BUBBLE,
    );
    fire("keydown");
    expect(key).toHaveBeenCalledTimes(1);
  });

  it("keeps slots of different nodes apart", () => {
    const second = { ...makeEl(), __node_id: 2 };
    const a = vi.fn<() => void>();
    const b = vi.fn<() => void>();
    set("onClick", a);
    set("onClickOnce", b, second);
    expect(core.addEventListener).toHaveBeenCalledTimes(2);

    fire("click", 2);
    fire("click", 2);
    expect(b).toHaveBeenCalledTimes(1);
    expect(a).not.toHaveBeenCalled();
    expect(core.removeEventListener).toHaveBeenCalledExactlyOnceWith(
      2,
      "click",
      expect.any(Function),
      BUBBLE,
    );

    fire("click", 1);
    expect(a).toHaveBeenCalledTimes(1);
  });

  it("skips a handler removed by an earlier handler during dispatch", () => {
    const log: string[] = [];
    const second = fn(log, "second");
    set("onClick", () => {
      log.push("first");
      patchProp(el, "onClickOnce", null, undefined, undefined, null);
    });
    set("onClickOnce", second);
    fire();
    fire();

    expect(second).not.toHaveBeenCalled();
    expect(log).toEqual(["first", "first"]);
  });

  it("runs a handler patched during dispatch with its new function", () => {
    const log: string[] = [];
    set("onClick", () => {
      patchProp(el, "onClickOnce", null, fn(log, "new"), undefined, null);
    });
    set("onClickOnce", fn(log, "old"));
    fire();

    expect(log).toEqual(["new"]);
  });

  it("does not re-enter a once handler that dispatches again from inside itself", () => {
    const log: string[] = [];
    set("onClick", fn(log, "plain"));
    set("onClickOnce", () => {
      log.push("once");
      fire();
    });
    fire();

    expect(log).toEqual(["plain", "once", "plain"]);
  });

  it.each([
    ["plain then once", ["onClick", "onClickOnce"], ["plain", "once", "plain", "plain"]],
    ["once then plain", ["onClickOnce", "onClick"], ["once", "plain", "plain", "plain"]],
  ])("fires the once handler one time and removes only itself (%s)", (_name, keys, expected) => {
    const log: string[] = [];
    for (const key of keys) set(key, fn(log, key === "onClick" ? "plain" : "once"));
    fire();
    fire();
    fire();

    expect(log).toEqual(expected);
    expect(core.addEventListener).toHaveBeenCalledTimes(1);
    expect(core.removeEventListener).not.toHaveBeenCalled();
  });

  describe("a fired .once listener", () => {
    it("stays spent when a re-render hands it a new function", () => {
      const first = vi.fn<() => void>();
      const next = vi.fn<() => void>();
      const plain = vi.fn<() => void>();
      set("onClick", plain);
      set("onClickOnce", first);
      fire();
      set("onClickOnce", next);
      fire();

      expect(first).toHaveBeenCalledTimes(1);
      expect(next).not.toHaveBeenCalled();
      expect(plain).toHaveBeenCalledTimes(2);
      expect(core.addEventListener).toHaveBeenCalledTimes(1);
      expect(core.removeEventListener).not.toHaveBeenCalled();
    });

    it("fires again once after its key is removed and added back", () => {
      const once = vi.fn<() => void>();
      const plain = vi.fn<() => void>();
      set("onClick", plain);
      set("onClickOnce", once);
      fire();
      patchProp(el, "onClickOnce", once, undefined, undefined, null);
      set("onClickOnce", once);
      fire();
      fire();

      expect(once).toHaveBeenCalledTimes(2);
      expect(plain).toHaveBeenCalledTimes(3);
      expect(core.addEventListener).toHaveBeenCalledTimes(1);
      expect(core.removeEventListener).not.toHaveBeenCalled();
    });

    it("registers with the host again when its key is removed and a live slot appears", () => {
      const once = vi.fn<() => void>();
      set("onClickOnce", once);
      fire();
      patchProp(el, "onClickOnce", once, undefined, undefined, null);
      expect(core.removeEventListener).toHaveBeenCalledTimes(1);
      set("onClickOnce", once);
      fire();

      expect(core.addEventListener).toHaveBeenCalledTimes(2);
      expect(once).toHaveBeenCalledTimes(2);
    });

    it("registers with the host again when a plain slot is added beside it", () => {
      const plain = vi.fn<() => void>();
      set("onClickOnce", vi.fn());
      fire();
      set("onClick", plain);
      fire();

      expect(core.addEventListener).toHaveBeenCalledTimes(2);
      expect(core.removeEventListener).toHaveBeenCalledTimes(1);
      expect(plain).toHaveBeenCalledTimes(1);
    });
  });

  it("runs a handler added under a new key during dispatch on the next dispatch only", () => {
    const late = vi.fn<() => void>();
    set("onClick", () => set("onClickOnce", late));
    fire();
    expect(late).not.toHaveBeenCalled();
    fire();
    expect(late).toHaveBeenCalledTimes(1);
  });

  describe("stopImmediatePropagation", () => {
    const makeEvent = () => {
      const original = vi.fn<() => void>();
      return { event: { stopImmediatePropagation: original }, original };
    };
    const fireWith = (event: unknown, name = "click", id = 1): void => {
      const calls = vi.mocked(core.addEventListener).mock.calls;
      listenerOf(calls.findLast(([nodeId, ev]) => nodeId === id && ev === name)!)(event);
    };
    const stopper = (log: string[], name: string) => (e: unknown) => {
      log.push(name);
      (e as { stopImmediatePropagation(): void }).stopImmediatePropagation();
    };

    it.each([
      ["plain then once", "onClick", "onClickOnce"],
      ["once then plain", "onClickOnce", "onClick"],
    ])("in the first slot stops the later slot (%s)", (_name, first, second) => {
      const log: string[] = [];
      const { event, original } = makeEvent();
      set(first, stopper(log, "first"));
      set(second, fn(log, "second"));
      fireWith(event);

      expect(log).toEqual(["first"]);
      expect(original).toHaveBeenCalledTimes(1);
    });

    it("does not stop slots of other events or nodes", () => {
      const log: string[] = [];
      const second = { ...makeEl(), __node_id: 2 };
      set("onClick", stopper(log, "click"));
      set("onKeydown", fn(log, "keydown"));
      set("onClick", fn(log, "other-node"), second);
      fireWith(makeEvent().event);
      fireWith(makeEvent().event, "keydown");
      fireWith(makeEvent().event, "click", 2);

      expect(log).toEqual(["click", "keydown", "other-node"]);
    });

    it("does not stop the next dispatch", () => {
      const log: string[] = [];
      let stop = true;
      set("onClick", (e: unknown) => {
        log.push("first");
        if (stop) stopper([], "")(e);
      });
      set("onClickOnce", fn(log, "second"));
      fireWith(makeEvent().event);
      stop = false;
      fireWith(makeEvent().event);

      expect(log).toEqual(["first", "first", "second"]);
    });

    it("in the last slot leaves every slot running", () => {
      const log: string[] = [];
      set("onClick", fn(log, "first"));
      set("onClickOnce", stopper(log, "last"));
      fireWith(makeEvent().event);

      expect(log).toEqual(["first", "last"]);
    });

    it("leaves no own method behind on an event that inherits it", () => {
      const event = Object.create({ stopImmediatePropagation: vi.fn<() => void>() }) as object;
      set("onClick", stopper([], "a"));
      fireWith(event);

      expect(Object.hasOwn(event, "stopImmediatePropagation")).toBe(false);
    });

    it.each([
      ["a non-object", "text"],
      ["null", null],
      ["no argument", undefined],
      ["an object without the method", {}],
    ])("does not crash on %s as the event", (_name, event) => {
      const log: string[] = [];
      set("onClick", fn(log, "a"));
      set("onClickOnce", fn(log, "b"));
      fireWith(event);

      expect(log).toEqual(["a", "b"]);
    });

    it.each([
      [
        "calls the original once per stop in a re-entrant dispatch",
        (_event: { stopImmediatePropagation: unknown }, original: unknown) =>
          expect(original).toHaveBeenCalledTimes(2),
      ],
      [
        "restores the original after a re-entrant dispatch",
        (event: { stopImmediatePropagation: unknown }, original: unknown) =>
          expect(event.stopImmediatePropagation).toBe(original),
      ],
    ])("%s", (_name, check) => {
      const log: string[] = [];
      const { event, original } = makeEvent();
      let nested = true;
      set("onClick", (e: unknown) => {
        log.push("first");
        if (nested) {
          nested = false;
          fireWith(e);
        }
        stopper([], "")(e);
      });
      set("onClickOnce", fn(log, "second"));
      fireWith(event);

      expect(log).toEqual(["first", "first"]);
      check(event, original);
    });

    it("stops the outer dispatch when the inner re-entrant dispatch stops", () => {
      const log: string[] = [];
      const { event, original } = makeEvent();
      let nested = true;
      set("onClick", (e: unknown) => {
        log.push("first");
        if (nested) {
          nested = false;
          fireWith(e);
        }
      });
      set("onClickOnce", stopper(log, "stopper"));
      set("onClickOnceOnce", fn(log, "last"));
      fireWith(event);

      expect(log).toEqual(["first", "first", "stopper"]);
      expect(original).toHaveBeenCalledTimes(1);
      expect(event.stopImmediatePropagation).toBe(original);
    });

    describe("when the original method throws", () => {
      const boom = new Error("stop failed");
      const original = vi.fn<() => void>(() => {
        throw boom;
      });
      const event = { stopImmediatePropagation: original };
      const log: string[] = [];
      beforeEach(() => {
        log.length = 0;
        set("onClick", stopper(log, "first"));
        set("onClickOnce", fn(log, "second"));
      });

      it("stops the later slot", () => {
        expect(() => fireWith(event)).toThrow(boom);
        expect(log).toEqual(["first"]);
      });

      it("reports the error to Vue", () => {
        expect(() => fireWith(event)).toThrow(boom);
        expect(warn).toHaveBeenCalledWith(expect.stringContaining("Unhandled error"));
      });

      it("restores the original method", () => {
        expect(() => fireWith(event)).toThrow(boom);
        expect(event.stopImmediatePropagation).toBe(original);
      });
    });

    it("does not crash on a frozen event and still runs every slot", () => {
      const log: string[] = [];
      set("onClick", fn(log, "a"));
      set("onClickOnce", fn(log, "b"));
      fireWith(Object.freeze({ stopImmediatePropagation: vi.fn<() => void>() }));

      expect(log).toEqual(["a", "b"]);
    });
  });

  describe("a throwing handler", () => {
    const boom = new Error("boom");
    const throwing = (): void => {
      throw boom;
    };

    it("stays registered for the next dispatch", () => {
      const log: string[] = [];
      set("onClick", throwing);
      set("onClickOnce", fn(log, "once"));

      expect(() => fire()).toThrow(boom);
      expect(() => fire()).toThrow(boom);
      expect(log).toEqual(["once"]);
    });

    it("still retires a throwing once handler", () => {
      const plain = vi.fn<() => void>();
      set("onClickOnce", throwing);
      set("onClick", plain);

      expect(() => fire()).toThrow(boom);
      fire();
      expect(plain).toHaveBeenCalledTimes(2);
      expect(core.removeEventListener).not.toHaveBeenCalled();
    });
  });

  it("leaves no slot behind when the host registration throws", () => {
    vi.mocked(core.addEventListener).mockImplementationOnce(() => {
      throw new Error("destroyed");
    });
    expect(() => set("onClick", vi.fn())).toThrow("destroyed");

    const plain = vi.fn<() => void>();
    set("onClick", plain);
    expect(core.addEventListener).toHaveBeenCalledTimes(2);
    fire();
    expect(plain).toHaveBeenCalledTimes(1);
  });
});
