// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import { layoutOf, scrollIntoViewNode, scrollNode, scrollOf } from "./layout.mts";

const native = {
  layoutOf: vi.fn<
    (
      nodeId: number,
    ) => readonly [number, number, number, number, number, number, number, number, number] | null
  >(() => null),
  scrollOf: vi.fn<(nodeId: number) => readonly [number, number, number, number] | null>(() => null),
  scrollNode: vi.fn<(nodeId: number, left: number | null, top: number | null) => void>(),
  scrollIntoViewNode: vi.fn<(nodeId: number, block: string, inline: string) => void>(),
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.stubGlobal("__inca_native__", nativeStub(native));
});

describe("wrapper functions forward to __inca_native__", () => {
  it("layoutOf", () => {
    native.layoutOf.mockReturnValueOnce([1, 2, 3, 4, 0, 5, 6, 7, 8]);
    expect(layoutOf(1)).toEqual([1, 2, 3, 4, 0, 5, 6, 7, 8]);
    expect(native.layoutOf).toHaveBeenCalledWith(1);
  });

  it("scrollOf", () => {
    native.scrollOf.mockReturnValueOnce([1, 2, 3, 4]);
    expect(scrollOf(1)).toEqual([1, 2, 3, 4]);
    expect(native.scrollOf).toHaveBeenCalledWith(1);
  });

  it("scrollNode", () => {
    scrollNode(1, 5, null);
    expect(native.scrollNode).toHaveBeenCalledWith(1, 5, null);
  });

  it("scrollIntoViewNode", () => {
    scrollIntoViewNode(1, "center", "end");
    expect(native.scrollIntoViewNode).toHaveBeenCalledWith(1, "center", "end");
  });
});
