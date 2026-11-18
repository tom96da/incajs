// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { adoptElement } from "../../tests/adopt.mts";
import { nativeStub } from "../../tests/setup.mts";
import * as rendererCore from "../rendererCore.mts";
import { createPatchProp } from "./patchProp.mts";
import type { RendererCore, Element } from "../rendererCore.mts";

const core = {
  setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  setStyle: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  removeStyle: vi.fn<(nodeId: number, key: string) => void>(),
};

const patchProp = createPatchProp(core as unknown as RendererCore);

/** An element holds the id of its node, which is all that patchProp reads. */
const makeEl = (): Element => ({ __node_id: 1 }) as unknown as Element;

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

  it("re-sends every remaining key after a removal, in object order", () => {
    const style = { flexDirection: "row", "flex-direction": "column", width: 1 };
    mountStyle(style);
    patchProp(
      el,
      "style",
      style,
      { flexDirection: "row", "flex-direction": "column" },
      undefined,
      null,
    );

    expect(core.removeStyle).toHaveBeenCalledExactlyOnceWith(1, "width");
    expect(core.setStyle).toHaveBeenCalledWith(1, "flexDirection", "row");
    expect(core.setStyle).toHaveBeenLastCalledWith(1, "flex-direction", "column");
    expect(core.setStyle).toHaveBeenCalledTimes(2);
  });

  it("applies an in-place change when prev and next are the same object", () => {
    const other = { ...el, __node_id: 3 };
    const style: Record<string, unknown> = { gap: 8, width: 1 };
    patchProp(other, "style", null, style, undefined, null);
    vi.clearAllMocks();

    style.gap = 9;
    patchProp(other, "style", style, style, undefined, null);

    expect(core.setStyle).toHaveBeenCalledExactlyOnceWith(3, "gap", 9);
    expect(core.removeStyle).not.toHaveBeenCalled();
  });

  it("removes a key deleted in place", () => {
    const other = { ...el, __node_id: 4 };
    const style: Record<string, unknown> = { gap: 8, width: 1 };
    patchProp(other, "style", null, style, undefined, null);
    vi.clearAllMocks();

    delete style.gap;
    patchProp(other, "style", style, style, undefined, null);

    expect(core.removeStyle).toHaveBeenCalledExactlyOnceWith(4, "gap");
    expect(core.setStyle).toHaveBeenCalledExactlyOnceWith(4, "width", 1);
  });
});

describe("patchProp on a string style", () => {
  const core = rendererCore;
  const setAttribute = vi.fn<(nodeId: number, key: string, value: unknown) => void>();
  const setStyle = vi.fn<(nodeId: number, key: string, value: unknown) => void>();
  const removeStyle = vi.fn<(nodeId: number, key: string) => void>();
  const patchProp = createPatchProp(core);

  /** A fresh element, so that no style bookkeeping carries over between tests. */
  const makeEl = (id: number) => adoptElement(id, "div");

  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal("__inca_native__", nativeStub({ setAttribute, setStyle, removeStyle } as never));
  });

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
