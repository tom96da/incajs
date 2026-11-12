// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import * as core from "../rendererCore.mts";
import { createPatchProp } from "./patchProp.mts";

const setAttribute = vi.fn<(nodeId: number, key: string, value: unknown) => void>();
const setStyle = vi.fn<(nodeId: number, key: string, value: unknown) => void>();
const removeStyle = vi.fn<(nodeId: number, key: string) => void>();
const patchProp = createPatchProp(core);

/** A fresh element, so that no style bookkeeping carries over between tests. */
const makeEl = (id: number) => core.createIncaElement(id, "element", "div");

beforeEach(() => {
  vi.clearAllMocks();
  globalThis.__inca_native__ = { setAttribute, setStyle, removeStyle } as never;
});

describe("patchProp on a string style", () => {
  it("applies the string as the style attribute", () => {
    patchProp(makeEl(1), "style", null, "color: red", undefined, null);

    expect(setAttribute).toHaveBeenCalledExactlyOnceWith(1, "style", "color: red");
  });

  it("applies a later string over an earlier one", () => {
    const el = makeEl(1);
    patchProp(el, "style", null, "color: red", undefined, null);
    patchProp(el, "style", "color: red", "gap: 8px", undefined, null);

    expect(setAttribute).toHaveBeenLastCalledWith(1, "style", "gap: 8px");
  });

  it("sends every entry of an object that follows a string", () => {
    const el = makeEl(1);
    patchProp(el, "style", null, { gap: 8 }, undefined, null);
    patchProp(el, "style", { gap: 8 }, "color: red", undefined, null);
    setStyle.mockClear();
    patchProp(el, "style", "color: red", { gap: 8 }, undefined, null);

    expect(setStyle).toHaveBeenCalledExactlyOnceWith(1, "gap", 8);
  });

  it("clears the string declarations before an object that follows", () => {
    const el = makeEl(1);
    patchProp(el, "style", null, "color: red", undefined, null);
    patchProp(el, "style", "color: red", { gap: 8 }, undefined, null);

    expect(setAttribute).toHaveBeenLastCalledWith(1, "style", "");
  });

  it("leaves the removal of sent entries to the string", () => {
    const el = makeEl(1);
    patchProp(el, "style", null, { gap: 8 }, undefined, null);
    patchProp(el, "style", { gap: 8 }, "color: red", undefined, null);

    expect(removeStyle).not.toHaveBeenCalled();
  });

  it("clears the declarations when a string becomes null", () => {
    const el = makeEl(1);
    patchProp(el, "style", null, "color: red", undefined, null);
    patchProp(el, "style", "color: red", null, undefined, null);

    expect(setAttribute).toHaveBeenLastCalledWith(1, "style", "");
  });

  it("clears the string only once", () => {
    const el = makeEl(1);
    patchProp(el, "style", null, "color: red", undefined, null);
    patchProp(el, "style", "color: red", null, undefined, null);
    patchProp(el, "style", null, null, undefined, null);

    expect(setAttribute).toHaveBeenCalledTimes(2);
  });

  it("sends the empty string", () => {
    patchProp(makeEl(1), "style", null, "", undefined, null);

    expect(setAttribute).toHaveBeenCalledExactlyOnceWith(1, "style", "");
  });

  it("clears the empty string before an object that follows", () => {
    const el = makeEl(1);
    patchProp(el, "style", null, "", undefined, null);
    patchProp(el, "style", "", { gap: 8 }, undefined, null);

    expect(setAttribute).toHaveBeenCalledTimes(2);
  });

  it("sends the same string again", () => {
    const el = makeEl(1);
    patchProp(el, "style", null, "color: red", undefined, null);
    patchProp(el, "style", "color: red", "color: red", undefined, null);

    expect(setAttribute).toHaveBeenCalledTimes(2);
  });
});
