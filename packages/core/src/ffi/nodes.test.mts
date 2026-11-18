// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import {
  appendChild,
  childAt,
  childCount,
  childNames,
  childNodeIds,
  cloneNode,
  compareNodePosition,
  createElementNS,
  insertAdjacentHtml,
  insertBefore,
  insertNodes,
  interfacePrototypes,
  isEqualNode,
  linkedNode,
  namedChild,
  nodeHandle,
  queryNode,
  queryNodes,
  registerUncloneable,
  removeChild,
  removeNode,
  replaceChild,
  replaceWithText,
  rootOf,
  setCharacterData,
  setInnerHtml,
  setOuterHtml,
  splitText,
  templateContent,
  textContent,
} from "./nodes.mts";
import type { ElementEntry, InsertionPlace, InterfacePrototypes, NodeLink } from "./native.mts";

const native = {
  interfacePrototypes: vi.fn<() => InterfacePrototypes>(),
  registerUncloneable: vi.fn<(map: WeakMap<object, unknown>) => void>(),
  createElementNS: vi.fn<(namespace: string | null, qualifiedName: string) => ElementEntry>(() => [
    1,
    "a",
    null,
    null,
    "a",
  ]),
  appendChild: vi.fn<(parentId: number, childId: number) => number>(() => 0),
  insertBefore: vi.fn<(parentId: number, childId: number, refId: number | null) => number>(() => 0),
  removeChild: vi.fn<(parentId: number, childId: number) => number[][]>(() => []),
  replaceChild: vi.fn<(parentId: number, nodeId: number, childId: number) => number[][]>(() => []),
  replaceWithText: vi.fn<(nodeId: number, text: string) => [number | null, number[][]]>(() => [
    null,
    [],
  ]),
  insertNodes: vi.fn<
    (
      nodeId: number,
      place: InsertionPlace,
      nodes: (number | string)[],
    ) => [number[], number[][]] | null
  >(() => null),
  cloneNode: vi.fn<(nodeId: number, deep: boolean) => number>(() => 0),
  nodeHandle: vi.fn<(nodeId: number) => ReturnType<typeof nodeHandle>>(() => null),
  rootOf: vi.fn<(nodeId: number) => number>((nodeId) => nodeId),
  compareNodePosition: vi.fn<(nodeId: number, otherId: number) => number>(() => 0),
  isEqualNode: vi.fn<(nodeId: number, otherId: number) => boolean>(() => false),
  removeNode: vi.fn<(nodeId: number) => [number[], number[]]>(() => [[], []]),
  templateContent: vi.fn<(nodeId: number) => number | null>(() => null),
  setInnerHtml: vi.fn<(nodeId: number, html: string) => number[][]>(() => []),
  setOuterHtml: vi.fn<(nodeId: number, html: string) => number[][]>(() => []),
  insertAdjacentHtml: vi.fn<(nodeId: number, position: string, html: string) => void>(),
  querySelector: vi.fn<(nodeId: number, selector: string) => number | null>(() => null),
  querySelectorAll: vi.fn<(nodeId: number, selector: string) => number[]>(() => []),
  linkedNode: vi.fn<(nodeId: number, link: NodeLink, elementOnly?: boolean) => number | null>(
    () => null,
  ),
  childNodeIds: vi.fn<(nodeId: number, elementOnly?: boolean) => number[]>(() => []),
  childCount: vi.fn<(nodeId: number, elementOnly: boolean) => number>(() => 0),
  childAt: vi.fn<(nodeId: number, index: number, elementOnly: boolean) => number | null>(
    () => null,
  ),
  namedChild: vi.fn<(nodeId: number, name: string) => number | null>(() => null),
  childNames: vi.fn<(nodeId: number) => string[]>(() => []),
  textContent: vi.fn<(nodeId: number) => string>(() => ""),
  setCharacterData: vi.fn<(nodeId: number, data: string) => void>(),
  splitText: vi.fn<(nodeId: number, offset: number) => number>(() => 0),
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.stubGlobal("__inca_native__", nativeStub(native));
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
    native.replaceChild.mockReturnValueOnce([[3, 4]]);
    expect(replaceChild(1, 2, 3)).toEqual([[3, 4]]);
    expect(native.replaceChild).toHaveBeenCalledWith(1, 2, 3);
  });

  it("replaceWithText", () => {
    native.replaceWithText.mockReturnValueOnce([5, [[2, 3]]]);
    expect(replaceWithText(1, "x")).toEqual([5, [[2, 3]]]);
    expect(native.replaceWithText).toHaveBeenCalledWith(1, "x");
  });

  it("interfacePrototypes", () => {
    const prototypes = { Node: {} };
    native.interfacePrototypes.mockReturnValueOnce(prototypes as never);
    expect(interfacePrototypes()).toBe(prototypes);
    expect(native.interfacePrototypes).toHaveBeenCalledWith();
  });

  it("registerUncloneable", () => {
    const map = new WeakMap<object, unknown>();
    registerUncloneable(map);
    expect(native.registerUncloneable).toHaveBeenCalledWith(map);
  });

  it("createElementNS", () => {
    native.createElementNS.mockReturnValueOnce([5, "Y", "urn:x", "x", "X:Y"]);
    expect(createElementNS("urn:x", "x:Y")).toEqual([5, "Y", "urn:x", "x", "X:Y"]);
    expect(native.createElementNS).toHaveBeenCalledWith("urn:x", "x:Y");
  });

  it("nodeHandle", () => {
    const result: ReturnType<typeof nodeHandle> = [{}, true, "input"];
    native.nodeHandle.mockReturnValueOnce(result);
    expect(nodeHandle(7)).toBe(result);
    expect(native.nodeHandle).toHaveBeenCalledWith(7);
  });

  it("cloneNode", () => {
    native.cloneNode.mockReturnValueOnce(7);
    expect(cloneNode(1, true)).toBe(7);
    expect(native.cloneNode).toHaveBeenCalledWith(1, true);
  });

  it("setInnerHtml", () => {
    native.setInnerHtml.mockReturnValueOnce([[3, 4]]);
    expect(setInnerHtml(1, "<b>")).toEqual([[3, 4]]);
    expect(native.setInnerHtml).toHaveBeenCalledWith(1, "<b>");
  });

  it("setOuterHtml", () => {
    native.setOuterHtml.mockReturnValueOnce([[1, 2]]);
    expect(setOuterHtml(1, "<b>")).toEqual([[1, 2]]);
    expect(native.setOuterHtml).toHaveBeenCalledWith(1, "<b>");
  });

  it("insertAdjacentHtml", () => {
    expect(insertAdjacentHtml(1, "afterend", "<b>")).toBeUndefined();
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
    native.removeChild.mockReturnValueOnce([[2, 3]]);
    expect(removeChild(1, 2)).toEqual([[2, 3]]);
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

  it("childCount", () => {
    native.childCount.mockReturnValueOnce(3);
    expect(childCount(1, true)).toBe(3);
    expect(native.childCount).toHaveBeenCalledWith(1, true);
  });

  it("childAt", () => {
    native.childAt.mockReturnValueOnce(7);
    expect(childAt(1, 2, false)).toBe(7);
    expect(native.childAt).toHaveBeenCalledWith(1, 2, false);
  });

  it("namedChild", () => {
    native.namedChild.mockReturnValueOnce(7);
    expect(namedChild(1, "x")).toBe(7);
    expect(native.namedChild).toHaveBeenCalledWith(1, "x");
  });

  it("childNames", () => {
    native.childNames.mockReturnValueOnce(["x"]);
    expect(childNames(1)).toEqual(["x"]);
    expect(native.childNames).toHaveBeenCalledWith(1);
  });

  it.each([["textContent", () => textContent(1), "text", [1]]] as const)(
    "%s",
    (name, call, result, args) => {
      native[name].mockReturnValueOnce(result);
      expect(call()).toBe(result);
      expect(native[name]).toHaveBeenCalledWith(...args);
    },
  );

  it("setCharacterData", () => {
    setCharacterData(1, "d");
    expect(native.setCharacterData).toHaveBeenCalledWith(1, "d");
  });

  it("splitText", () => {
    native.splitText.mockReturnValueOnce(7);
    expect(splitText(1, 2)).toBe(7);
    expect(native.splitText).toHaveBeenCalledWith(1, 2);
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

  it("insertNodes forwards its arguments and gives the lists the host returns", () => {
    native.insertNodes.mockReturnValueOnce([[7], [[8, 9]]]);

    expect(insertNodes(1, "before", [2, "s"])).toEqual([[7], [[8, 9]]]);
    expect(native.insertNodes).toHaveBeenCalledWith(1, "before", [2, "s"]);
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
