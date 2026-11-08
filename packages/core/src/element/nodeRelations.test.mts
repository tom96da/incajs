// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import { document } from "../document.mts";
import { adoptTextNode, createIncaElement, createIncaFragment } from "./handle.mts";
import { markDestroyed } from "./lifecycle.mts";

/** The id `nativeStub` gives the document. */
const DOCUMENT = 1;
/** The mask `compareNodePosition` answers, set per test. */
let mask = 0;
/** The top `rootOf` answers, set per test. */
let top = DOCUMENT;

const host = {
  rootOf: vi.fn<(nodeId: number) => number>(() => top),
  compareNodePosition: vi.fn<(nodeId: number, otherId: number) => number>(() => mask),
  isEqualNode: vi.fn<(nodeId: number, otherId: number) => boolean>(() => true),
};

beforeEach(() => {
  vi.clearAllMocks();
  mask = 0;
  top = DOCUMENT;
  globalThis.__inca_native__ = nativeStub(host as never);
});

const div = (id: number) => createIncaElement(id, "element", "div");

describe("contains", () => {
  it("is true for the node itself without asking the host", () => {
    const node = div(10);

    expect(node.contains(node)).toBe(true);
    expect(host.compareNodePosition).not.toHaveBeenCalled();
  });

  it("is true when the host reports the node as contained by this one", () => {
    mask = 16 | 4;

    expect(div(10).contains(div(11))).toBe(true);
  });

  it("asks the host for the position of the other node", () => {
    div(10).contains(div(11));

    expect(host.compareNodePosition).toHaveBeenCalledWith(10, 11);
  });

  it.each([
    ["an unrelated node", 0],
    ["an ancestor", 8 | 2],
    ["a disconnected node", 1 | 32 | 4],
  ])("is false for %s", (_name, answer) => {
    mask = answer;

    expect(div(10).contains(div(11))).toBe(false);
  });

  it.each([
    ["null", null],
    ["undefined", undefined],
  ])("is false for %s", (_name, value) => {
    expect(div(10).contains(value as null)).toBe(false);
  });

  it("is false for a freed node", () => {
    const node = div(10);
    mask = 16;
    markDestroyed(node);

    expect(node.contains(div(11))).toBe(false);
  });

  it("is true for a freed node and itself", () => {
    const node = div(10);
    markDestroyed(node);

    expect(node.contains(node)).toBe(true);
  });

  it("is false for a freed other node", () => {
    const other = div(11);
    mask = 16;
    markDestroyed(other);

    expect(div(10).contains(other)).toBe(false);
  });

  it.each([
    ["a number", 5],
    ["a plain object", { parent: null }],
  ])("throws a TypeError for %s", (_name, value) => {
    expect(() => div(10).contains(value as unknown as null)).toThrow(TypeError);
  });
});

describe("getRootNode", () => {
  it.each([
    ["a connected node", () => div(10)],
    ["the document", () => document],
  ])("gives document for %s", (_name, make) => {
    expect(make().getRootNode()).toBe(document);
  });

  it("gives the top node of a detached subtree", () => {
    const outer = div(20);
    top = 20;

    expect(div(10).getRootNode()).toBe(outer);
  });

  it("gives the fragment that holds the node", () => {
    const group = createIncaFragment(21);
    top = 21;

    expect(adoptTextNode(10, "x").getRootNode()).toBe(group);
  });

  it("gives a freed node itself without asking the host", () => {
    const node = div(10);
    markDestroyed(node);

    expect(node.getRootNode()).toBe(node);
    expect(host.rootOf).not.toHaveBeenCalled();
  });
});

describe("compareDocumentPosition", () => {
  it("returns the mask of the host", () => {
    mask = 20;

    expect(div(10).compareDocumentPosition(div(11))).toBe(20);
  });

  it("asks the host for the pair in the order of the call", () => {
    div(10).compareDocumentPosition(div(11));

    expect(host.compareNodePosition).toHaveBeenCalledWith(10, 11);
  });

  it("gives disconnected and preceding for a freed node and a lower other id", () => {
    const node = div(10);
    markDestroyed(node);

    expect(node.compareDocumentPosition(div(9))).toBe(1 | 32 | 2);
    expect(host.compareNodePosition).not.toHaveBeenCalled();
  });

  it("gives disconnected and following for a freed other node with a higher id", () => {
    const other = div(11);
    markDestroyed(other);

    expect(div(10).compareDocumentPosition(other)).toBe(1 | 32 | 4);
    expect(host.compareNodePosition).not.toHaveBeenCalled();
  });

  it("gives 0 for the same freed node", () => {
    const node = div(10);
    markDestroyed(node);

    expect(node.compareDocumentPosition(node)).toBe(0);
    expect(host.compareNodePosition).not.toHaveBeenCalled();
  });

  it.each([
    ["no argument", []],
    ["null", [null]],
    ["a number", [5]],
  ])("throws a TypeError for %s", (_name, args) => {
    const node = div(10) as unknown as { compareDocumentPosition(...args: unknown[]): number };

    expect(() => node.compareDocumentPosition(...args)).toThrow(TypeError);
  });
});

describe("isSameNode", () => {
  it("is true for the node itself", () => {
    const node = div(10);

    expect(node.isSameNode(node)).toBe(true);
  });

  it("is false for another node", () => {
    expect(div(10).isSameNode(div(11))).toBe(false);
  });

  it.each([
    ["null", null],
    ["undefined", undefined],
  ])("is false for %s", (_name, value) => {
    expect(div(10).isSameNode(value as null)).toBe(false);
  });

  it("throws a TypeError for a value that is not a node", () => {
    expect(() => div(10).isSameNode(5 as unknown as null)).toThrow(TypeError);
  });
});

describe("isEqualNode", () => {
  it("returns the answer of the host", () => {
    host.isEqualNode.mockReturnValueOnce(false);

    expect(div(10).isEqualNode(div(11))).toBe(false);
  });

  it("asks the host for the pair in the order of the call", () => {
    div(10).isEqualNode(div(11));

    expect(host.isEqualNode).toHaveBeenCalledWith(10, 11);
  });

  it.each([
    ["null", null],
    ["undefined", undefined],
  ])("is false for %s without asking the host", (_name, value) => {
    expect(div(10).isEqualNode(value as null)).toBe(false);
    expect(host.isEqualNode).not.toHaveBeenCalled();
  });

  it("is false for a freed node without asking the host", () => {
    const node = div(10);
    markDestroyed(node);

    expect(node.isEqualNode(div(11))).toBe(false);
    expect(host.isEqualNode).not.toHaveBeenCalled();
  });

  it("is false for a freed other node without asking the host", () => {
    const other = div(11);
    markDestroyed(other);

    expect(div(10).isEqualNode(other)).toBe(false);
    expect(host.isEqualNode).not.toHaveBeenCalled();
  });

  it("is true for a freed node compared to itself without asking the host", () => {
    const node = div(10);
    markDestroyed(node);

    expect(node.isEqualNode(node)).toBe(true);
    expect(host.isEqualNode).not.toHaveBeenCalled();
  });

  it("throws a TypeError for a value that is not a node", () => {
    expect(() => div(10).isEqualNode(5 as unknown as null)).toThrow(TypeError);
  });
});
