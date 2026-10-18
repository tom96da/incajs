// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import * as core from "../rendererCore.mts";
import { createPatchProp } from "./patchProp.mts";
import type { IncaElement } from "../rendererCore.mts";

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
    setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
    removeAttribute: vi.fn<(nodeId: number, key: string) => void>(),
    getAttribute: vi.fn<() => null>(() => null),
  };
  globalThis.__inca_native__ = native as never;
  return native;
}

let native = installNative();
const patchProp = createPatchProp(core);

/** Runs the registrations of `event` in order, as the host does. */
function fire(el: IncaElement, event = "click"): void {
  const payload = { type: event, currentTarget: el, eventId: 1, preventDefault() {} };
  for (const registration of registrations) {
    if (registration.event === event) globalThis.__inca_callbacks__[registration.id]?.(payload);
  }
}

beforeEach(() => {
  delete (globalThis as { __inca_callbacks__?: unknown }).__inca_callbacks__;
  registrations = [];
  native = installNative();
});

describe("patchProp on a handler property key", () => {
  const element = () => core.createIncaElement(1, "element", "div");

  it.each([
    ["a function", () => {}],
    ["null", null],
  ])("assigns %s to the element's property", (_name, value) => {
    const el = element();
    el.onclick = () => {};
    patchProp(el, "onclick", undefined, value);
    expect(el.onclick).toBe(value);
    expect(native.setAttribute).not.toHaveBeenCalled();
    expect(native.removeAttribute).not.toHaveBeenCalled();
  });

  it("clears the handler when a string replaces a function", () => {
    const el = element();
    patchProp(el, "onclick", undefined, () => {});
    patchProp(el, "onclick", undefined, "count++");
    expect(el.onclick).toBeNull();
    expect(native.setAttribute).toHaveBeenCalledTimes(1);
    expect(native.setAttribute).toHaveBeenCalledWith(1, "onclick", "count++");
  });

  it("sets an attribute for a lowercase on key the element lacks", () => {
    const el = element();
    const handler = () => {};
    patchProp(el, "onfocusin", undefined, handler);
    expect(native.setAttribute).toHaveBeenCalledWith(1, "onfocusin", String(handler));
  });

  it("sets an attribute for a function on a member that is no handler", () => {
    const el = element();
    const handler = () => {};
    patchProp(el, "focus", undefined, handler);
    expect(native.setAttribute).toHaveBeenCalledWith(1, "focus", String(handler));
    expect(Reflect.get(el, "focus")).not.toBe(handler);
  });

  it("runs a Vue listener and the handler property in the order bound", () => {
    const el = element();
    const log: string[] = [];
    patchProp(el, "onClick", undefined, () => log.push("vue"));
    patchProp(el, "onclick", undefined, () => log.push("property"));
    fire(el);
    expect(log).toEqual(["vue", "property"]);
  });
});
