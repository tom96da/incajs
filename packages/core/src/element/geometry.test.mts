// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it } from "vitest";

import { adoptElement } from "../../tests/adopt.mts";
import { installFakeHost } from "../../tests/fakeHost.mts";
import { collectGarbage } from "../../tests/gc.mts";
import { keep } from "../../tests/keep.mts";
import { installMembersHost } from "../../tests/setup.mts";
import { createElementNode, rootElement } from "./handle.mts";
import { markFreed } from "./lifecycle.mts";

type Box = readonly [number, number, number, number, number, number, number, number, number];

describe("offsetParent", () => {
  it("is null when there is no record", () => {
    installMembersHost();

    expect(adoptElement(1, "div").offsetParent).toBeNull();
  });

  it("is the parent element that has a box", () => {
    installMembersHost(
      { 1: [100, 50, 200, 100, 0, 0, 0, 0, 0], 2: [110, 60, 20, 20, 0, 0, 0, 0, 0] },
      null,
      { 2: 1 },
    );
    const parent = adoptElement(1, "div");
    const child = adoptElement(2, "div");

    expect(child.offsetParent).toBe(parent);
  });

  it("is null when the parent has no record", () => {
    installMembersHost({ 2: [110, 60, 20, 20, 0, 0, 0, 0, 0] }, null, { 2: 1 });
    const child = adoptElement(2, "div");
    keep(adoptElement(1, "div"));

    expect(child.offsetParent).toBeNull();
  });

  it("is null for the root wrapper", () => {
    installMembersHost({ 1: [5.4, 6, 10, 10, 0, 0, 0, 0, 0] });

    expect(adoptElement(1, "div").offsetParent).toBeNull();
  });

  it("is null for a freed element and reads the record of the host for a removed one", () => {
    installMembersHost({ 1: [10, 20, 30, 40, 2, 0, 0, 0, 0] });
    const el = adoptElement(1, "div");
    markFreed([el.__node_id]);

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
    boxes[host.ROOT] = [0, 0, 200, 100, 0, 0, 0, 0, 0];
    boxes[top.__node_id] = [10, 10, 20, 20, 0, 0, 0, 0, 0];

    await collectGarbage(() => root.deref() === undefined);
    expect(root.deref()).toBeUndefined();

    expect(top.offsetParent?.__node_id).toBe(host.ROOT);
  });
});
