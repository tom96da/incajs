// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { installFakeHost } from "../../tests/fakeHost.mts";
import { collectGarbage } from "../../tests/gc.mts";
import { installMembersHost, nativeStub } from "../../tests/setup.mts";
import { offsetOf, rectOf, tabIndexOf } from "./geometry.mts";
import { createElementNode, createIncaElement, rootElement } from "./handle.mts";
import { markFreed, nodeOf } from "./lifecycle.mts";
import type { HandleBuilders } from "./lifecycle.mts";

const builders = { node: nodeOf } as HandleBuilders;

type Box = readonly [number, number, number, number, number, number, number];

function installHost(
  boxes: Record<number, Box>,
  scrollOf = vi.fn<() => null>(() => null),
  parents: Record<number, number> = {},
): void {
  globalThis.__inca_native__ = nativeStub({
    linkedNode: (nodeId: number, link: string) =>
      link === "parent" ? (parents[nodeId] ?? null) : null,
    layoutOf: vi.fn<(nodeId: number) => Box | null>((nodeId) => boxes[nodeId] ?? null),
    scrollOf,
  } as never);
}

describe("offsetOf", () => {
  it("measures from the parent's padding edge inside its border", () => {
    installHost({ 1: [100, 50, 200, 100, 4, 0, 0], 2: [110, 60, 20, 20, 0, 0, 0] }, undefined, {
      2: 1,
    });
    createIncaElement(1, "element", "div");
    const child = createIncaElement(2, "element", "div");

    expect(offsetOf(child, builders)).toEqual({ left: 6, top: 6 });
  });

  it("adds the scroll recorded with the box and ignores a later write", () => {
    const scrollOf = vi.fn<() => readonly [number, number, number, number]>(() => [90, 90, 0, 300]);
    installHost(
      { 1: [100, 50, 200, 100, 4, 0, 0], 2: [95, 20, 20, 20, 0, 15, 40] },
      scrollOf as never,
      { 2: 1 },
    );
    createIncaElement(1, "element", "div");
    const child = createIncaElement(2, "element", "div");

    expect(offsetOf(child, builders)).toEqual({ left: 6, top: 6 });
    expect(scrollOf).not.toHaveBeenCalled();
  });
});

describe("rectOf", () => {
  it("derives the edges from the box and keeps fractional values", () => {
    const rect = rectOf([10.5, 20.25, 30.75, 40, 2, 0, 0]);
    expect([rect.x, rect.y, rect.width, rect.height]).toEqual([10.5, 20.25, 30.75, 40]);
    expect([rect.left, rect.top, rect.right, rect.bottom]).toEqual([10.5, 20.25, 41.25, 60.25]);
  });

  it("returns the plain values from toJSON", () => {
    const rect = rectOf([1, 2, 3, 4, 0, 0, 0]);
    expect(rect.toJSON()).toEqual({
      x: 1,
      y: 2,
      width: 3,
      height: 4,
      top: 2,
      left: 1,
      right: 4,
      bottom: 6,
    });
  });

  it("gives all zeros for a null layout", () => {
    const rect = rectOf(null);
    expect([
      rect.x,
      rect.y,
      rect.width,
      rect.height,
      rect.top,
      rect.left,
      rect.right,
      rect.bottom,
    ]).toEqual([0, 0, 0, 0, 0, 0, 0, 0]);
  });
});

describe("tabIndexOf", () => {
  it.each<[string | number | boolean | null, string, number]>([
    ["0", "div", 0],
    ["5", "div", 5],
    ["  7", "div", 7],
    ["\t\n\f\r 9", "div", 9],
    ["+3", "div", 3],
    ["-4", "div", -4],
    ["-0", "div", 0],
    ["12abc", "div", 12],
    ["1.5", "div", 1],
    [1.5, "div", 1],
    ["2147483647", "div", 2147483647],
    ["-2147483648", "div", -2147483648],
    ["2147483648", "div", -1],
    ["-2147483649", "div", -1],
    [1e10, "div", -1],
    [NaN, "div", -1],
    ["", "div", -1],
    [" ", "div", -1],
    ["\u000b5", "div", -1],
    ["+", "div", -1],
    ["-", "div", -1],
    ["abc", "div", -1],
    [null, "div", -1],
    [true, "div", -1],
    [false, "div", -1],
    [null, "button", 0],
    [null, "input", 0],
    [true, "button", 0],
    ["x", "button", 0],
    ["3", "button", 3],
  ])(
    "parses integers and applies the default on invalid input (%o, %s)",
    (value, kind, expected) => {
      expect(tabIndexOf(value, kind)).toBe(expected);
    },
  );
});

describe("createIncaElement layout", () => {
  it("reads the rect from the last drawn box", () => {
    installMembersHost({ 1: [10, 20, 30, 40, 2, 0, 0] });
    const el = createIncaElement(1, "element", "div");
    const rect = el.getBoundingClientRect();
    expect([
      rect.x,
      rect.y,
      rect.width,
      rect.height,
      rect.left,
      rect.top,
      rect.right,
      rect.bottom,
    ]).toEqual([10, 20, 30, 40, 10, 20, 40, 60]);
  });

  it("gives zero members and a null offsetParent when there is no record", () => {
    installMembersHost();
    const el = createIncaElement(1, "element", "div");
    expect([el.offsetWidth, el.offsetHeight, el.offsetLeft, el.offsetTop]).toEqual([0, 0, 0, 0]);
    expect([el.clientWidth, el.clientHeight, el.clientLeft, el.clientTop]).toEqual([0, 0, 0, 0]);
    expect(el.offsetParent).toBeNull();
  });

  it("rounds the offset and client members", () => {
    installMembersHost({ 1: [10.4, 20.6, 30.5, 40.2, 1.6, 0, 0] });
    const el = createIncaElement(1, "element", "div");
    expect([el.offsetWidth, el.offsetHeight]).toEqual([31, 40]);
    expect([el.offsetLeft, el.offsetTop]).toEqual([10, 21]);
    expect([el.clientLeft, el.clientTop]).toEqual([2, 2]);
    expect([el.clientWidth, el.clientHeight]).toEqual([27, 37]);
  });

  it("reads a position slightly above the parent as 0", () => {
    installMembersHost(
      {
        1: [100, 50, 200, 100, 0, 0, 0],
        2: [99.7, 49.7, 20, 20, 0, 0, 0],
      },
      null,
      { 2: 1 },
    );
    createIncaElement(1, "element", "div");
    const child = createIncaElement(2, "element", "div");

    expect([child.offsetLeft, child.offsetTop]).toEqual([0, 0]);
  });

  it("clamps the client size at zero", () => {
    installMembersHost({ 1: [0, 0, 2, 2, 5, 0, 0] });
    const el = createIncaElement(1, "element", "div");
    expect([el.clientWidth, el.clientHeight]).toEqual([0, 0]);
  });

  it("measures offsets from the offsetParent", () => {
    installMembersHost(
      {
        1: [100, 50, 200, 100, 0, 0, 0],
        2: [110.4, 60.6, 20, 20, 0, 0, 0],
      },
      null,
      { 2: 1 },
    );
    const parent = createIncaElement(1, "element", "div");
    const child = createIncaElement(2, "element", "div");

    expect(child.offsetParent).toBe(parent);
    expect([child.offsetLeft, child.offsetTop]).toEqual([10, 11]);
  });

  it("gives a null offsetParent when the parent has no record", () => {
    installMembersHost({ 2: [110, 60, 20, 20, 0, 0, 0] }, null, { 2: 1 });
    const child = createIncaElement(2, "element", "div");
    createIncaElement(1, "element", "div");

    expect(child.offsetParent).toBeNull();
    expect([child.offsetLeft, child.offsetTop]).toEqual([110, 60]);
  });

  it("gives a null offsetParent to the root wrapper", () => {
    installMembersHost({ 1: [5.4, 6, 10, 10, 0, 0, 0] });
    const el = createIncaElement(1, "element", "div");
    expect(el.offsetParent).toBeNull();
    expect([el.offsetLeft, el.offsetTop]).toEqual([5, 6]);
  });

  it("reads zeros for a freed element", () => {
    installMembersHost({ 1: [10, 20, 30, 40, 2, 0, 0] });
    const el = createIncaElement(1, "element", "div");
    markFreed([el.__node_id]);
    expect([
      el.getBoundingClientRect().width,
      el.offsetWidth,
      el.clientWidth,
      el.offsetLeft,
    ]).toEqual([0, 0, 0, 0]);
    expect(el.offsetParent).toBeNull();
  });
});

describe("offsetParent of a top-level element", () => {
  it("is the mount container after the handle of the container was collected", async () => {
    const boxes: Record<number, Box> = {};
    const host = installFakeHost({ layoutOf: (nodeId: number) => boxes[nodeId] ?? null });
    const top = createElementNode("div");
    const root = (() => {
      const container = rootElement();
      container.appendChild(top);
      return new WeakRef(container);
    })();
    boxes[host.ROOT] = [0, 0, 200, 100, 0, 0, 0];
    boxes[top.__node_id] = [10, 10, 20, 20, 0, 0, 0];

    await collectGarbage(() => root.deref() === undefined);
    expect(root.deref()).toBeUndefined();

    expect(top.offsetParent?.__node_id).toBe(host.ROOT);
  });
});
