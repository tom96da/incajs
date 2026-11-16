// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import { HTML_NAMESPACE } from "./native.mts";
import {
  appendChild,
  checkSelector,
  childNodeIds,
  cloneNode,
  compareNodePosition,
  createElementNS,
  innerHtml,
  insertAdjacentHtml,
  insertBefore,
  interfacePrototypes,
  isConnected,
  isEqualNode,
  linkedNode,
  nameOf,
  nameOfRow,
  outerHtml,
  queryNode,
  queryNodes,
  removeChild,
  removeNode,
  replaceChild,
  replaceData,
  rootOf,
  setInnerHtml,
  setOuterHtml,
  splitText,
  substringData,
  templateContent,
  textContent,
  wholeText,
} from "./nodes.mts";
import type { ElementEntry, InterfacePrototypes, NodeEntry, NodeLink, Parsed } from "./native.mts";

const native = {
  interfacePrototypes: vi.fn<() => InterfacePrototypes>(),
  createElementNS: vi.fn<(namespace: string | null, qualifiedName: string) => ElementEntry>(() => [
    1,
    "a",
    null,
    null,
    "a",
  ]),
  appendChild: vi.fn<(parentId: number, childId: number) => number>(() => 0),
  insertBefore: vi.fn<(parentId: number, childId: number, refId: number | null) => number>(() => 0),
  removeChild: vi.fn<(parentId: number, childId: number) => void>(),
  replaceChild: vi.fn<(parentId: number, nodeId: number, childId: number) => number>(() => 0),
  cloneNode: vi.fn<(nodeId: number, deep: boolean) => NodeEntry[]>(() => []),
  rootOf: vi.fn<(nodeId: number) => number>((nodeId) => nodeId),
  compareNodePosition: vi.fn<(nodeId: number, otherId: number) => number>(() => 0),
  isEqualNode: vi.fn<(nodeId: number, otherId: number) => boolean>(() => false),
  removeNode: vi.fn<(nodeId: number) => [number[], number[]]>(() => [[], []]),
  templateContent: vi.fn<(nodeId: number) => number | null>(() => null),
  innerHtml: vi.fn<(nodeId: number) => string>(() => ""),
  outerHtml: vi.fn<(nodeId: number) => string>(() => ""),
  setInnerHtml: vi.fn<(nodeId: number, html: string) => Parsed>(() => [0, []]),
  setOuterHtml: vi.fn<(nodeId: number, html: string) => Parsed | null>(() => null),
  insertAdjacentHtml: vi.fn<(nodeId: number, position: string, html: string) => Parsed>(() => [
    0,
    [],
  ]),
  checkSelector: vi.fn<(selector: string) => void>(),
  querySelector: vi.fn<(nodeId: number, selector: string) => number | null>(() => null),
  querySelectorAll: vi.fn<(nodeId: number, selector: string) => number[]>(() => []),
  linkedNode: vi.fn<(nodeId: number, link: NodeLink, elementOnly?: boolean) => number | null>(
    () => null,
  ),
  childNodeIds: vi.fn<(nodeId: number, elementOnly?: boolean) => number[]>(() => []),
  isConnected: vi.fn<(nodeId: number) => boolean>(() => true),
  textContent: vi.fn<(nodeId: number) => string>(() => ""),
  wholeText: vi.fn<(nodeId: number) => string>(() => ""),
  substringData: vi.fn<(nodeId: number, offset: number, count: number) => string>(() => ""),
  replaceData: vi.fn<(nodeId: number, offset: number, count: number, data: string) => void>(),
  splitText: vi.fn<(nodeId: number, offset: number) => number>(() => 0),
};

beforeEach(() => {
  vi.clearAllMocks();
  globalThis.__inca_native__ = nativeStub(native);
});

describe("wrapper functions forward to __inca_native__", () => {
  it("appendChild", () => {
    native.appendChild.mockReturnValueOnce(4);
    expect(appendChild(1, 2)).toBe(4);
    expect(native.appendChild).toHaveBeenCalledWith(1, 2);
  });

  it("insertBefore", () => {
    native.insertBefore.mockReturnValueOnce(5);
    expect(insertBefore(1, 2, 3)).toBe(5);
    expect(native.insertBefore).toHaveBeenCalledWith(1, 2, 3);
  });

  it("replaceChild", () => {
    native.replaceChild.mockReturnValueOnce(6);
    expect(replaceChild(1, 2, 3)).toBe(6);
    expect(native.replaceChild).toHaveBeenCalledWith(1, 2, 3);
  });

  it("interfacePrototypes", () => {
    const prototypes = { Node: {} };
    native.interfacePrototypes.mockReturnValueOnce(prototypes as never);
    expect(interfacePrototypes()).toBe(prototypes);
    expect(native.interfacePrototypes).toHaveBeenCalledWith();
  });

  it("createElementNS", () => {
    native.createElementNS.mockReturnValueOnce([5, "Y", "urn:x", "x", "X:Y"]);
    expect(createElementNS("urn:x", "x:Y")).toEqual([5, "Y", "urn:x", "x", "X:Y"]);
    expect(native.createElementNS).toHaveBeenCalledWith("urn:x", "x:Y");
  });

  it("cloneNode", () => {
    native.cloneNode.mockReturnValueOnce([[7, "#text", "x"]]);
    expect(cloneNode(1, true)).toEqual([[7, "#text", "x"]]);
    expect(native.cloneNode).toHaveBeenCalledWith(1, true);
  });

  it.each([
    ["innerHtml", () => innerHtml(1), "<b></b>"],
    ["outerHtml", () => outerHtml(1), "<p><b></b></p>"],
  ] as const)("%s", (name, call, result) => {
    native[name].mockReturnValueOnce(result);
    expect(call()).toBe(result);
    expect(native[name]).toHaveBeenCalledWith(1);
  });

  it("setInnerHtml", () => {
    native.setInnerHtml.mockReturnValueOnce([0, [[2, "b", HTML_NAMESPACE, 9, null]]]);
    expect(setInnerHtml(1, "<b>")).toEqual([0, [[2, "b", HTML_NAMESPACE, 9, null]]]);
    expect(native.setInnerHtml).toHaveBeenCalledWith(1, "<b>");
  });

  it("setOuterHtml", () => {
    native.setOuterHtml.mockReturnValueOnce(null);
    expect(setOuterHtml(1, "<b>")).toBeNull();
    expect(native.setOuterHtml).toHaveBeenCalledWith(1, "<b>");
  });

  it("insertAdjacentHtml", () => {
    native.insertAdjacentHtml.mockReturnValueOnce([3, []]);
    expect(insertAdjacentHtml(1, "afterend", "<b>")).toEqual([3, []]);
    expect(native.insertAdjacentHtml).toHaveBeenCalledWith(1, "afterend", "<b>");
  });

  it("rootOf", () => {
    native.rootOf.mockReturnValueOnce(9);
    expect(rootOf(1)).toBe(9);
    expect(native.rootOf).toHaveBeenCalledWith(1);
  });

  it("compareNodePosition", () => {
    native.compareNodePosition.mockReturnValueOnce(20);
    expect(compareNodePosition(1, 2)).toBe(20);
    expect(native.compareNodePosition).toHaveBeenCalledWith(1, 2);
  });

  it("isEqualNode", () => {
    native.isEqualNode.mockReturnValueOnce(true);
    expect(isEqualNode(1, 2)).toBe(true);
    expect(native.isEqualNode).toHaveBeenCalledWith(1, 2);
  });

  it("removeChild", () => {
    removeChild(1, 2);
    expect(native.removeChild).toHaveBeenCalledWith(1, 2);
  });

  it("linkedNode", () => {
    native.linkedNode.mockReturnValueOnce(7);
    expect(linkedNode(1, "next")).toBe(7);
    expect(native.linkedNode).toHaveBeenCalledWith(1, "next", undefined);
  });

  it("linkedNode passes the element flag", () => {
    linkedNode(1, "next", true);
    expect(native.linkedNode).toHaveBeenCalledWith(1, "next", true);
  });

  it("childNodeIds", () => {
    native.childNodeIds.mockReturnValueOnce([2, 3]);
    expect(childNodeIds(1)).toEqual([2, 3]);
    expect(native.childNodeIds).toHaveBeenCalledWith(1, undefined);
  });

  it("childNodeIds passes the element flag", () => {
    childNodeIds(1, true);
    expect(native.childNodeIds).toHaveBeenCalledWith(1, true);
  });

  it.each([
    ["textContent", () => textContent(1), "text", [1]],
    ["wholeText", () => wholeText(1), "whole", [1]],
    ["substringData", () => substringData(1, 2, 3), "part", [1, 2, 3]],
  ] as const)("%s", (name, call, result, args) => {
    native[name].mockReturnValueOnce(result);
    expect(call()).toBe(result);
    expect(native[name]).toHaveBeenCalledWith(...args);
  });

  it("replaceData", () => {
    replaceData(1, 2, 3, "d");
    expect(native.replaceData).toHaveBeenCalledWith(1, 2, 3, "d");
  });

  it("splitText", () => {
    native.splitText.mockReturnValueOnce(7);
    expect(splitText(1, 2)).toBe(7);
    expect(native.splitText).toHaveBeenCalledWith(1, 2);
  });

  it("isConnected", () => {
    native.isConnected.mockReturnValueOnce(false);
    expect(isConnected(1)).toBe(false);
    expect(native.isConnected).toHaveBeenCalledWith(1);
  });

  it("the checkSelector wrapper forwards the selector", () => {
    checkSelector("div > p");
    expect(native.checkSelector).toHaveBeenCalledWith("div > p");
  });

  it("the queryNode wrapper forwards the id and the selector", () => {
    native.querySelector.mockReturnValueOnce(7);

    expect(queryNode(1, "p")).toBe(7);
    expect(native.querySelector).toHaveBeenCalledWith(1, "p");
  });

  it("the queryNodes wrapper forwards the id and the selector", () => {
    native.querySelectorAll.mockReturnValueOnce([7, 8]);

    expect(queryNodes(2, "span")).toEqual([7, 8]);
    expect(native.querySelectorAll).toHaveBeenCalledWith(2, "span");
  });

  it("templateContent gives the fragment id the host reports", () => {
    native.templateContent.mockReturnValueOnce(8);
    expect(templateContent(1)).toBe(8);
    expect(native.templateContent).toHaveBeenCalledWith(1);
  });

  it("removeNode gives the callback ids and the node ids the host reports", () => {
    native.removeNode.mockReturnValueOnce([
      [4, 5],
      [1, 2],
    ]);
    expect(removeNode(1)).toEqual([
      [4, 5],
      [1, 2],
    ]);
    expect(native.removeNode).toHaveBeenCalledWith(1);
  });
});

describe("nameOf", () => {
  it("gives the tag of a two-part entry", () => {
    expect(nameOf([7, "div"])).toBe("div");
  });

  it("gives the four name parts of a five-part entry", () => {
    expect(nameOf([7, "Y", "urn:x", "x", "X:Y"])).toEqual(["Y", "urn:x", "x", "X:Y"]);
  });
});

describe("nameOfRow", () => {
  it("gives the tag of an HTML row", () => {
    expect(nameOfRow([7, "div", HTML_NAMESPACE, 0, null])).toBe("div");
  });

  it("gives the tuple of a foreign row without upper case or prefix", () => {
    expect(nameOfRow([7, "foreignObject", "urn:x", 0, null])).toEqual([
      "foreignObject",
      "urn:x",
      null,
      "foreignObject",
    ]);
  });
});
