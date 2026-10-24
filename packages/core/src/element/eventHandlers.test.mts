// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { createIncaElement } from "./handle.mts";
import { markDestroyed } from "./lifecycle.mts";
import type { IncaElement } from "./types.mts";

let registrations: { event: string; id: number }[] = [];

/** Installs a fake host that numbers the listeners it registers, and returns it. */
function installNative() {
  const native = {
    addEventListener: vi.fn<
      (nodeId: number, event: string, capture: boolean, passive: boolean) => number
    >((_nodeId, event) => {
      const id = registrations.length + 1;
      registrations.push({ event, id });
      return id;
    }),
    removeEventListener: vi.fn<(nodeId: number, event: string, callbackId: number) => boolean>(
      () => true,
    ),
    getAttribute: vi.fn<() => null>(() => null),
  };
  globalThis.__inca_native__ = native as never;
  return native;
}

let native = installNative();

/** Runs the registrations of `event` in order, as the host does, and returns the payload. */
function fire(el: IncaElement, event = "click") {
  const payload = { type: event, currentTarget: el, preventDefault: vi.fn<() => void>() };
  for (const registration of registrations) {
    if (registration.event === event) globalThis.__inca_callbacks__[registration.id]?.(payload);
  }
  return payload;
}

beforeEach(() => {
  delete (globalThis as { __inca_callbacks__?: unknown }).__inca_callbacks__;
  registrations = [];
  native = installNative();
});

describe("createIncaElement handler properties", () => {
  const element = () => createIncaElement(1, "element", "div");

  it("has one for each event the host dispatches that has one in HTML", () => {
    const names = Object.getOwnPropertyNames(element())
      .filter((n) => /^on[a-z]/.test(n))
      .sort();
    expect(names).toEqual([
      "onauxclick",
      "onblur",
      "onclick",
      "oncontextmenu",
      "ondblclick",
      "onfocus",
      "ongotpointercapture",
      "onkeydown",
      "onkeyup",
      "onlostpointercapture",
      "onmousedown",
      "onmouseenter",
      "onmouseleave",
      "onmousemove",
      "onmouseout",
      "onmouseover",
      "onmouseup",
      "onpointerdown",
      "onpointerenter",
      "onpointerleave",
      "onpointermove",
      "onpointerout",
      "onpointerover",
      "onpointerup",
      "onscroll",
      "onwheel",
    ]);
  });

  it("reads null before a handler is set", () => {
    expect(element().onclick).toBeNull();
  });

  it("reads back the function it was given", () => {
    const el = element();
    const fn = vi.fn<() => void>();
    el.onclick = fn;
    expect(el.onclick).toBe(fn);
  });

  it("reads null after a value that is no function or object", () => {
    const el = element();
    el.onclick = () => {};
    Reflect.set(el, "onclick", "count++");
    expect(el.onclick).toBeNull();
  });

  it("keeps an object it was given", () => {
    const el = element();
    const object = { handleEvent: vi.fn<() => void>() };
    Reflect.set(el, "onclick", object);
    expect(el.onclick).toBe(object);
    fire(el);
    expect(object.handleEvent).not.toHaveBeenCalled();
    expect(native.addEventListener).toHaveBeenCalledTimes(1);
  });

  it("registers one bubble listener with the host on the first function", () => {
    const el = element();
    el.onclick = () => {};
    el.onclick = () => {};
    expect(native.addEventListener).toHaveBeenCalledTimes(1);
    expect(native.addEventListener).toHaveBeenCalledWith(1, "click", false, false);
  });

  it("runs at the place of its first assignment among the listeners", () => {
    const el = element();
    const log: string[] = [];
    el.addEventListener("click", () => log.push("first"));
    el.onclick = () => log.push("handler");
    el.addEventListener("click", () => log.push("second"));
    fire(el);
    expect(log).toEqual(["first", "handler", "second"]);
  });

  it("keeps its place when a later function replaces it", () => {
    const el = element();
    const log: string[] = [];
    el.onclick = () => log.push("h1");
    el.addEventListener("click", () => log.push("listener"));
    el.onclick = () => log.push("h2");
    fire(el);
    expect(log).toEqual(["h2", "listener"]);
  });

  it("moves to the end when a function follows null", () => {
    const el = element();
    const log: string[] = [];
    el.onclick = () => log.push("h1");
    el.addEventListener("click", () => log.push("listener"));
    el.onclick = null;
    el.onclick = () => log.push("h2");
    fire(el);
    expect(log).toEqual(["listener", "h2"]);
  });

  it("removes its listener while it holds null", () => {
    const el = element();
    el.onclick = () => {};
    el.onclick = null;
    expect(native.removeEventListener).toHaveBeenCalledTimes(1);
    expect(native.removeEventListener).toHaveBeenCalledWith(1, "click", registrations[0]!.id);
  });

  it("cancels the event when the handler returns false", () => {
    const el = element();
    el.onclick = () => false;
    expect(fire(el).preventDefault).toHaveBeenCalledTimes(1);
  });

  it("leaves the event uncanceled when the handler returns another value", () => {
    const el = element();
    el.onclick = () => {};
    expect(fire(el).preventDefault).not.toHaveBeenCalled();
  });

  it("calls the handler on the element with the event", () => {
    const el = element();
    const calls: { self: unknown; args: unknown[] }[] = [];
    el.onclick = function (this: unknown, ...args: unknown[]) {
      calls.push({ self: this, args });
    };
    const payload = fire(el);
    expect(calls).toHaveLength(1);
    expect(calls[0]!.self).toBe(el);
    expect(calls[0]!.args).toHaveLength(1);
    expect(calls[0]!.args[0]).toBe(payload);
  });

  it("lets an error the handler throws reach the host", () => {
    const el = element();
    const boom = new Error("boom");
    el.onclick = () => {
      throw boom;
    };
    expect(() => fire(el)).toThrow(boom);
  });

  it("ignores an assignment after the node is removed", () => {
    const el = element();
    markDestroyed(el);
    el.onclick = () => {};
    expect(el.onclick).toBeNull();
    expect(native.addEventListener).not.toHaveBeenCalled();
  });
});
