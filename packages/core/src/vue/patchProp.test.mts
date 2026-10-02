// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from "vitest";
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
    patchProp(el, "onClick", null, vi.fn(), undefined, null);
    patchProp(el, "onClick", vi.fn(), undefined, undefined, null);

    expect(core.setEventListener).toHaveBeenCalledTimes(1);
    expect(core.removeEventListener).toHaveBeenCalledExactlyOnceWith(1, "click");
  });

  it("does nothing when unbinding an event that was never bound", () => {
    patchProp(el, "onClick", vi.fn(), undefined, undefined, null);

    expect(core.setEventListener).not.toHaveBeenCalled();
    expect(core.removeEventListener).not.toHaveBeenCalled();
  });
});

describe("on* modifiers", () => {
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

describe("plain and .once listeners on one element", () => {
  const set = (key: string, value: unknown, target = el): void =>
    patchProp(target, key, null, value, undefined, null);
  // Runs the most recent host registration for (id, event).
  const fire = (event = "click", id = 1): void => {
    const calls = vi.mocked(core.setEventListener).mock.calls;
    calls.findLast(([nodeId, name]) => nodeId === id && name === event)![2]();
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

  it("registers a plain listener alone once and keeps it", () => {
    const plain = vi.fn<() => void>();
    set("onClick", plain);
    fire();
    fire();

    expect(core.setEventListener).toHaveBeenCalledTimes(1);
    expect(plain).toHaveBeenCalledTimes(2);
    expect(core.removeEventListener).not.toHaveBeenCalled();
  });

  it("fires a once listener alone one time and unbinds the event", () => {
    const once = vi.fn<() => void>();
    set("onClickOnce", once);
    const registered = vi.mocked(core.setEventListener).mock.calls[0]![2];
    registered();
    registered();

    expect(once).toHaveBeenCalledTimes(1);
    expect(core.removeEventListener).toHaveBeenCalledExactlyOnceWith(1, "click");
  });

  it("registers one host listener for both and runs them in registration order", () => {
    const log: string[] = [];
    const plain = fn(log, "plain");
    const once = fn(log, "once");
    set("onClick", plain);
    set("onClickOnce", once);

    expect(core.setEventListener).toHaveBeenCalledTimes(1);
    fire();
    expect(log).toEqual(["plain", "once"]);
  });

  it("runs once-then-plain registration in that order", () => {
    const log: string[] = [];
    set("onClickOnce", fn(log, "once"));
    set("onClick", fn(log, "plain"));

    expect(core.setEventListener).toHaveBeenCalledTimes(1);
    fire();
    expect(log).toEqual(["once", "plain"]);
  });

  it("unbinds the host only when the last slot goes, after a once fired", () => {
    const plain = vi.fn<() => void>();
    set("onClick", plain);
    set("onClickOnce", vi.fn());
    fire();
    expect(core.removeEventListener).not.toHaveBeenCalled();

    patchProp(el, "onClick", plain, undefined, undefined, null);
    expect(core.removeEventListener).toHaveBeenCalledExactlyOnceWith(1, "click");
  });

  it("re-registers with the host after every slot went away", () => {
    set("onClick", vi.fn());
    patchProp(el, "onClick", null, undefined, undefined, null);
    set("onClick", vi.fn());

    expect(core.setEventListener).toHaveBeenCalledTimes(2);
    expect(core.removeEventListener).toHaveBeenCalledTimes(1);
  });

  it("replaces a plain handler in place without touching the once slot or the host", () => {
    const log: string[] = [];
    set("onClick", fn(log, "plain-1"));
    set("onClickOnce", fn(log, "once"));
    patchProp(el, "onClick", null, fn(log, "plain-2"), undefined, null);
    fire();

    expect(core.setEventListener).toHaveBeenCalledTimes(1);
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
    expect(core.removeEventListener).toHaveBeenCalledExactlyOnceWith(1, "click");
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
    set("onClickPassive", other);
    // A key change unpatches the old key, then patches the new one.
    patchProp(el, "onClick", toggled, undefined, undefined, null);
    patchProp(el, "onClickOnce", undefined, toggled, undefined, null);
    fire();
    fire();

    expect(log).toEqual(["other", "toggled", "other"]);
    expect(core.setEventListener).toHaveBeenCalledTimes(1);
    expect(core.removeEventListener).not.toHaveBeenCalled();
  });

  it("toggles a lone handler from once to plain, re-registering with the host", () => {
    const handler = vi.fn<() => void>();
    set("onClickOnce", handler);
    patchProp(el, "onClickOnce", handler, undefined, undefined, null);
    patchProp(el, "onClick", undefined, handler, undefined, null);
    fire();
    fire();

    expect(handler).toHaveBeenCalledTimes(2);
    expect(core.setEventListener).toHaveBeenCalledTimes(2);
    expect(core.removeEventListener).toHaveBeenCalledTimes(1);
  });

  it("registers a distinct event on the same node separately", () => {
    const click = vi.fn<() => void>();
    const key = vi.fn<() => void>();
    set("onClick", click);
    set("onKeydown", key);
    set("onClickOnce", vi.fn());

    expect(core.setEventListener).toHaveBeenCalledTimes(2);
    fire("keydown");
    expect(key).toHaveBeenCalledTimes(1);
    expect(click).not.toHaveBeenCalled();
  });

  it("leaves another event untouched when a slot goes away", () => {
    const key = vi.fn<() => void>();
    set("onClick", vi.fn());
    set("onKeydown", key);
    patchProp(el, "onClick", null, undefined, undefined, null);

    expect(core.removeEventListener).toHaveBeenCalledExactlyOnceWith(1, "click");
    fire("keydown");
    expect(key).toHaveBeenCalledTimes(1);
  });

  it("keeps slots of different nodes apart", () => {
    const second = { ...makeEl(), id: 2 };
    const a = vi.fn<() => void>();
    const b = vi.fn<() => void>();
    set("onClick", a);
    set("onClickOnce", b, second);
    expect(core.setEventListener).toHaveBeenCalledTimes(2);

    fire("click", 2);
    fire("click", 2);
    expect(b).toHaveBeenCalledTimes(1);
    expect(a).not.toHaveBeenCalled();
    expect(core.removeEventListener).toHaveBeenCalledExactlyOnceWith(2, "click");

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
    expect(core.setEventListener).toHaveBeenCalledTimes(1);
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
      expect(core.setEventListener).toHaveBeenCalledTimes(1);
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
      expect(core.setEventListener).toHaveBeenCalledTimes(1);
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

      expect(core.setEventListener).toHaveBeenCalledTimes(2);
      expect(once).toHaveBeenCalledTimes(2);
    });

    it("registers with the host again when a plain slot is added beside it", () => {
      const plain = vi.fn<() => void>();
      set("onClickOnce", vi.fn());
      fire();
      set("onClick", plain);
      fire();

      expect(core.setEventListener).toHaveBeenCalledTimes(2);
      expect(core.removeEventListener).toHaveBeenCalledTimes(1);
      expect(plain).toHaveBeenCalledTimes(1);
    });

    it("is dropped from the host when the remaining plain slot goes too", () => {
      set("onClick", vi.fn());
      set("onClickOnce", vi.fn());
      fire();
      patchProp(el, "onClick", null, undefined, undefined, null);

      expect(core.removeEventListener).toHaveBeenCalledExactlyOnceWith(1, "click");
    });
  });

  it("runs a handler added under a new key during dispatch on the next dispatch only", () => {
    const late = vi.fn<() => void>();
    set("onClick", () => set("onClickPassive", late));
    fire();
    expect(late).not.toHaveBeenCalled();
    fire();
    expect(late).toHaveBeenCalledTimes(1);
  });

  it("runs every function of an array value inside a slot", () => {
    const log: string[] = [];
    set("onClick", [fn(log, "a"), fn(log, "b")]);
    set("onClickOnce", fn(log, "once"));
    fire();

    expect(log).toEqual(["a", "b", "once"]);
  });

  describe("stopImmediatePropagation", () => {
    const makeEvent = () => {
      const original = vi.fn<() => void>();
      return { event: { stopImmediatePropagation: original }, original };
    };
    const fireWith = (event: unknown, name = "click", id = 1): void => {
      const calls = vi.mocked(core.setEventListener).mock.calls;
      calls.findLast(([nodeId, ev]) => nodeId === id && ev === name)![2](event);
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

    it("inside an array value stops the later functions and the later slot", () => {
      const log: string[] = [];
      const { event, original } = makeEvent();
      set("onClick", [stopper(log, "a"), fn(log, "b")]);
      set("onClickOnce", fn(log, "once"));
      fireWith(event);

      expect(log).toEqual(["a"]);
      expect(original).toHaveBeenCalledTimes(1);
    });

    it("does not stop slots of other events or nodes", () => {
      const log: string[] = [];
      const second = { ...makeEl(), id: 2 };
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

    it("in the last slot changes nothing but still reaches the host", () => {
      const log: string[] = [];
      const { event, original } = makeEvent();
      set("onClick", fn(log, "first"));
      set("onClickOnce", stopper(log, "last"));
      fireWith(event);

      expect(log).toEqual(["first", "last"]);
      expect(original).toHaveBeenCalledTimes(1);
    });

    it("restores the event's own method after dispatch", () => {
      const { event, original } = makeEvent();
      set("onClick", stopper([], "a"));
      fireWith(event);

      expect(event.stopImmediatePropagation).toBe(original);
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

    it("calls the original once per stop and restores it after a re-entrant dispatch", () => {
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
      expect(original).toHaveBeenCalledTimes(2);
      expect(event.stopImmediatePropagation).toBe(original);
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
      set("onClickPassive", fn(log, "last"));
      fireWith(event);

      expect(log).toEqual(["first", "first", "stopper"]);
      expect(original).toHaveBeenCalledTimes(1);
      expect(event.stopImmediatePropagation).toBe(original);
    });

    it("sets the flag, reports to Vue and restores when the original method throws", () => {
      const log: string[] = [];
      const boom = new Error("stop failed");
      const original = vi.fn<() => void>(() => {
        throw boom;
      });
      const event = { stopImmediatePropagation: original };
      set("onClick", stopper(log, "first"));
      set("onClickOnce", fn(log, "second"));

      expect(() => fireWith(event)).toThrow(boom);
      expect(log).toEqual(["first"]);
      expect(warn).toHaveBeenCalledWith(expect.stringContaining("Unhandled error"));
      expect(event.stopImmediatePropagation).toBe(original);
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

    it("lets the other slot run, then rethrows the first error and reports to Vue", () => {
      const log: string[] = [];
      set("onClick", throwing);
      set("onClickOnce", fn(log, "once"));

      expect(() => fire()).toThrow(boom);
      expect(log).toEqual(["once"]);
      expect(() => fire()).toThrow(boom);
      expect(log).toEqual(["once"]);
      expect(warn).toHaveBeenCalledWith(expect.stringContaining("Unhandled error"));
    });

    it("rethrows a failing handler's error to the host when several fail", () => {
      const second = new Error("second");
      set("onClick", throwing);
      set("onClickOnce", () => {
        throw second;
      });

      expect(() => fire()).toThrow(boom);
      expect(warn).toHaveBeenCalledTimes(2);
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

    it("writes nothing to the console when no handler throws", () => {
      set("onClick", vi.fn());
      set("onClickOnce", vi.fn());
      fire();

      expect(warn).not.toHaveBeenCalled();
      expect(error).not.toHaveBeenCalled();
    });
  });

  it("leaves no slot behind when the host registration throws", () => {
    vi.mocked(core.setEventListener).mockImplementationOnce(() => {
      throw new Error("destroyed");
    });
    expect(() => set("onClick", vi.fn())).toThrow("destroyed");

    const plain = vi.fn<() => void>();
    set("onClick", plain);
    expect(core.setEventListener).toHaveBeenCalledTimes(2);
    fire();
    expect(plain).toHaveBeenCalledTimes(1);
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

  let warn: MockInstance<typeof console.warn>;
  beforeEach(() => {
    warn = vi.spyOn(console, "warn").mockImplementation(() => {});
  });
  afterEach(() => {
    warn.mockRestore();
  });

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
    expect(warn).toHaveBeenCalledWith(expect.stringContaining("Unhandled error"));
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

  it("rethrows a failing handler's error to the host when an array of handlers fails", () => {
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
    });

    async function register(mode: string, instance: ComponentInternalInstance | null) {
      vi.stubEnv("NODE_ENV", mode);
      patchProp(el, "onClick", null, () => Promise.reject(boom), undefined, instance);
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
