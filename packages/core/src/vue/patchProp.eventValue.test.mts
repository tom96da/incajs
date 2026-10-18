// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from "vitest";

import * as rendererCore from "../rendererCore.mts";
import { createPatchProp } from "./patchProp.mts";
import type { IncaCore, IncaElement } from "../rendererCore.mts";

const core: IncaCore = {
  ...rendererCore,
  addEventListener: vi.fn<IncaCore["addEventListener"]>(),
  removeEventListener: vi.fn<IncaCore["removeEventListener"]>(),
  latestEventId: vi.fn<() => number>(() => 0),
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

describe("onXxx values", () => {
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
    const fn = vi.fn();
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
    const handler = vi.fn();
    patchProp(el, "onClick", null, [null, handler], undefined, null);
    registered()();

    expect(handler).toHaveBeenCalledOnce();
    expect(warn).not.toHaveBeenCalled();
  });

  it("runs the functions of a nested array", () => {
    const handler = vi.fn();
    patchProp(el, "onClick", null, [[handler]], undefined, null);
    registered()();

    expect(handler).toHaveBeenCalledOnce();
  });

  it("warns at dispatch about an array entry that is not a function", () => {
    const handler = vi.fn();
    patchProp(el, "onClick", null, ["x", handler], undefined, null);
    registered()();

    expect(handler).toHaveBeenCalledOnce();
    expect(warn).toHaveBeenCalledOnce();
  });
});

describe("with a production build of Vue", () => {
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
