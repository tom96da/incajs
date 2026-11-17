// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { createRenderer, h, nextTick, ref } from "@vue/runtime-core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import * as rendererCore from "../rendererCore.mts";
import { createNodeOps } from "./nodeOps.mts";
import { createPatchProp } from "./patchProp.mts";
import type { IncaCore, IncaElement } from "../rendererCore.mts";

const core: IncaCore = {
  ...rendererCore,
  setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  removeAttribute: vi.fn<(nodeId: number, key: string) => void>(),
};

const patchProp = createPatchProp(core);

/** An element holds the id of its node, which is all that patchProp reads. */
const makeEl = (): IncaElement => ({ __node_id: 1 }) as unknown as IncaElement;

let el = makeEl();

beforeEach(() => {
  vi.clearAllMocks();
  el = makeEl();
});

describe("everything else", () => {
  it.each([
    ["label", "hello"],
    ["count", 3],
    ["foo", true],
  ])("forwards %s=%p to setAttribute", (key, value) => {
    patchProp(el, key, null, value, undefined, null);

    expect(core.setAttribute).toHaveBeenCalledWith(1, key, value);
  });

  it("sets a true boolean attribute as an empty string", () => {
    patchProp(el, "disabled", null, true, undefined, null);

    expect(core.setAttribute).toHaveBeenCalledExactlyOnceWith(1, "disabled", "");
  });

  it("sets a boolean attribute that holds a non-empty string as an empty string", () => {
    patchProp(el, "hidden", null, "yes", undefined, null);

    expect(core.setAttribute).toHaveBeenCalledExactlyOnceWith(1, "hidden", "");
  });

  it.each(["until-found", "Until-Found"])("keeps hidden=%s as the string", (value) => {
    patchProp(el, "hidden", null, value, undefined, null);

    expect(core.setAttribute).toHaveBeenCalledExactlyOnceWith(1, "hidden", value);
  });

  it.each([
    ["an object", { nested: true }, "[object Object]"],
    ["a symbol", Symbol("tag"), "Symbol(tag)"],
  ])("sets %s as its string form", (_name, value, expected) => {
    patchProp(el, "data", null, value, undefined, null);

    expect(core.setAttribute).toHaveBeenCalledExactlyOnceWith(1, "data", expected);
    expect(core.removeAttribute).not.toHaveBeenCalled();
  });

  it("replaces a string with the string form of an object that follows it", () => {
    patchProp(el, "data", null, "old", undefined, null);
    patchProp(el, "data", "old", { nested: true }, undefined, null);

    expect(core.setAttribute).toHaveBeenLastCalledWith(1, "data", "[object Object]");
    expect(core.removeAttribute).not.toHaveBeenCalled();
  });

  it.each([null, undefined])("removes the attribute when the value turns %s", (value) => {
    patchProp(el, "label", null, "hello", undefined, null);
    patchProp(el, "label", "hello", value, undefined, null);

    expect(core.removeAttribute).toHaveBeenCalledExactlyOnceWith(1, "label");
  });

  it("removes an attribute that is absent from the new vnode props", async () => {
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
    const shown = ref(true);
    renderer
      .createApp({ render: () => h("view", shown.value ? { label: "hello" } : {}) })
      .mount(el);
    expect(core.setAttribute).toHaveBeenCalledExactlyOnceWith(2, "label", "hello");

    shown.value = false;
    await nextTick();

    expect(core.removeAttribute).toHaveBeenCalledExactlyOnceWith(2, "label");
  });

  it.each([
    "disabled",
    "hidden",
    "inert",
    "allowfullscreen",
    "formnovalidate",
    "ismap",
    "itemscope",
    "nomodule",
    "novalidate",
    "readonly",
  ])("removes %s when it turns false", (name) => {
    patchProp(el, name, true, false, undefined, null);

    expect(core.removeAttribute).toHaveBeenCalledExactlyOnceWith(1, name);
    expect(core.setAttribute).not.toHaveBeenCalled();
  });

  it("removes a boolean attribute that turns 0", () => {
    patchProp(el, "hidden", true, 0, undefined, null);

    expect(core.removeAttribute).toHaveBeenCalledExactlyOnceWith(1, "hidden");
    expect(core.setAttribute).not.toHaveBeenCalled();
  });

  it("keeps a boolean attribute set to an empty string", () => {
    patchProp(el, "hidden", null, "", undefined, null);

    expect(core.setAttribute).toHaveBeenCalledExactlyOnceWith(1, "hidden", "");
    expect(core.removeAttribute).not.toHaveBeenCalled();
  });

  it("keeps a false boolean on any other name as an attribute", () => {
    patchProp(el, "aria-hidden", true, false, undefined, null);

    expect(core.setAttribute).toHaveBeenCalledWith(1, "aria-hidden", false);
    expect(core.removeAttribute).not.toHaveBeenCalled();
  });
});

describe("patchProp on the value key", () => {
  const core = rendererCore;
  const setAttribute = vi.fn<(nodeId: number, key: string, value: unknown) => void>();
  const setInputValue = vi.fn<(nodeId: number, value: string) => void>();
  const patchProp = createPatchProp(core);

  beforeEach(() => {
    setAttribute.mockClear();
    setInputValue.mockClear();
    vi.stubGlobal("__inca_native__", nativeStub({ setAttribute, setInputValue } as never));
  });

  it.each<[unknown, string]>([
    ["a", "a"],
    [undefined, ""],
  ])("assigns value on an input (%o)", (value, expected) => {
    patchProp(core.createIncaElement(1, "input"), "value", undefined, value);
    expect(setInputValue).toHaveBeenCalledWith(1, expected);
    expect(setAttribute).not.toHaveBeenCalled();
  });

  it("sends value on other elements to setAttribute", () => {
    patchProp(core.createIncaElement(1, "div"), "value", undefined, "a");
    expect(setAttribute).toHaveBeenCalledWith(1, "value", "a");
    expect(setInputValue).not.toHaveBeenCalled();
  });
});

describe("patchProp on a handler property key", () => {
  const core = rendererCore;
  /** What the host calls: it passes the event object first. */
  type Callback = (...args: unknown[]) => void;

  let registrations: { event: string; id: number; callback: Callback }[] = [];
  let nextId = 1;

  /** Installs a fake host that numbers the listeners it registers, and returns it. */
  function installNative() {
    const native = {
      addEventListener: vi.fn<
        (
          nodeId: number,
          event: string,
          callback: Callback,
          capture: boolean,
          passive: boolean,
        ) => number
      >((_nodeId, event, callback) => {
        const id = nextId++;
        registrations.push({ event, id, callback });
        return id;
      }),
      removeEventListener: vi.fn<(nodeId: number, event: string, callbackId: number) => boolean>(
        (_nodeId, _event, callbackId) => {
          registrations = registrations.filter((registration) => registration.id !== callbackId);
          return true;
        },
      ),
      setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
      removeAttribute: vi.fn<(nodeId: number, key: string) => void>(),
      getAttribute: vi.fn<() => null>(() => null),
    };
    vi.stubGlobal("__inca_native__", nativeStub(native as never));
    return native;
  }

  let native = installNative();
  const patchProp = createPatchProp(core);

  /** Runs the registrations of `event` in order, as the host does. */
  function fire(el: IncaElement, event = "click"): void {
    const payload = { type: event, currentTarget: el, eventId: 1, preventDefault() {} };
    for (const registration of registrations) {
      if (registration.event === event) registration.callback(payload);
    }
  }

  beforeEach(() => {
    registrations = [];
    nextId = 1;
    native = installNative();
  });

  const element = () => core.createIncaElement(1, "div");

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
