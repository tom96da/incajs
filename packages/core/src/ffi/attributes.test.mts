// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import {
  ariaElements,
  attributeKey,
  getAttribute,
  inputValue,
  removeAttribute,
  removeStyle,
  setAriaElements,
  setInputValue,
} from "./attributes.mts";

const native = {
  ariaElements: vi.fn<(nodeId: number, attribute: string) => number[] | null>(() => null),
  setAriaElements: vi.fn<(nodeId: number, attribute: string, ids: number[] | null) => void>(),
  attributeKey: vi.fn<(nodeId: number, name: string) => string>((_nodeId, name) => name),
  getAttribute: vi.fn<(nodeId: number, key: string) => string | number | boolean | null>(
    () => null,
  ),
  removeAttribute: vi.fn<(nodeId: number, key: string) => void>(),
  removeStyle: vi.fn<(nodeId: number, key: string) => void>(),
  inputValue: vi.fn<(nodeId: number) => string>(),
  setInputValue: vi.fn<(nodeId: number, value: string) => void>(),
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.stubGlobal("__inca_native__", nativeStub(native));
});

describe("wrapper functions forward to __inca_native__", () => {
  it("attributeKey", () => {
    native.attributeKey.mockReturnValueOnce("viewbox");
    expect(attributeKey(1, "viewBox")).toBe("viewbox");
    expect(native.attributeKey).toHaveBeenCalledWith(1, "viewBox");
  });

  it("getAttribute", () => {
    native.getAttribute.mockReturnValueOnce("hello");
    expect(getAttribute(1, "label")).toBe("hello");
    expect(native.getAttribute).toHaveBeenCalledWith(1, "label");
  });

  it("ariaElements", () => {
    native.ariaElements.mockReturnValueOnce([4, 5]);
    expect(ariaElements(1, "aria-owns")).toEqual([4, 5]);
    expect(native.ariaElements).toHaveBeenCalledWith(1, "aria-owns");
  });

  it("setAriaElements", () => {
    setAriaElements(1, "aria-owns", [4]);
    expect(native.setAriaElements).toHaveBeenCalledWith(1, "aria-owns", [4]);
  });

  it("removeAttribute", () => {
    removeAttribute(1, "label");
    expect(native.removeAttribute).toHaveBeenCalledWith(1, "label");
  });

  it("removeStyle", () => {
    removeStyle(1, "display");
    expect(native.removeStyle).toHaveBeenCalledWith(1, "display");
  });

  it("inputValue", () => {
    native.inputValue.mockReturnValueOnce("abc");
    expect(inputValue(1)).toBe("abc");
    expect(native.inputValue).toHaveBeenCalledWith(1);
  });

  it("setInputValue", () => {
    setInputValue(1, "abc");
    expect(native.setInputValue).toHaveBeenCalledWith(1, "abc");
  });
});
