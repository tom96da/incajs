// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import * as core from "../rendererCore.mts";
import { createPatchProp } from "./patchProp.mts";

const setAttribute = vi.fn<(nodeId: number, key: string, value: unknown) => void>();
const setInputValue = vi.fn<(nodeId: number, value: string) => void>();
const patchProp = createPatchProp(core);

beforeEach(() => {
  setAttribute.mockClear();
  setInputValue.mockClear();
  globalThis.__inca_native__ = { setAttribute, setInputValue } as never;
});

describe("patchProp on the value key", () => {
  it.each<[unknown, string]>([
    ["a", "a"],
    [undefined, ""],
  ])("assigns value on an input (%o)", (value, expected) => {
    patchProp(core.createIncaElement(1, "element", "input"), "value", undefined, value);
    expect(setInputValue).toHaveBeenCalledWith(1, expected);
    expect(setAttribute).not.toHaveBeenCalled();
  });

  it("sends value on other elements to setAttribute", () => {
    patchProp(core.createIncaElement(1, "element", "div"), "value", undefined, "a");
    expect(setAttribute).toHaveBeenCalledWith(1, "value", "a");
    expect(setInputValue).not.toHaveBeenCalled();
  });
});
