// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { nativeStub } from "../tests/setup.mts";
import {
  appendChild,
  insertBefore,
  replaceChild,
  cloneNode,
  innerHtml,
  outerHtml,
  setInnerHtml,
  setOuterHtml,
  insertAdjacentHtml,
  rootOf,
  compareNodePosition,
  isEqualNode,
  checkSelector,
  focusedNode,
  childNodeIds,
  getAttribute,
  isConnected,
  replaceData,
  splitText,
  substringData,
  textContent,
  wholeText,
  layoutOf,
  linkedNode,
  scrollIntoViewNode,
  scrollNode,
  scrollOf,
  removeAttribute,
  removeChild,
  removeStyle,
  queryNode,
  queryNodes,
  inputValue,
  setInputValue,
} from "./tree.mts";
import type { HostCallback, NodeLink, Parsed } from "./native.mts";
import type { EventLike } from "./types.mts";

// Ids come from the host, which numbers them in order.
let nextCallbackId = 0;

const native = {
  rootNodeId: vi.fn<() => number>(() => 42),
  createNode: vi.fn<(tag: string) => number>((_tag: string) => 1),
  appendChild: vi.fn<(parentId: number, childId: number) => number>(() => 0),
  insertBefore: vi.fn<(parentId: number, childId: number, refId: number | null) => number>(() => 0),
  removeChild: vi.fn<(parentId: number, childId: number) => void>(),
  replaceChild: vi.fn<(parentId: number, nodeId: number, childId: number) => number>(() => 0),
  cloneNode: vi.fn<(nodeId: number, deep: boolean) => [number, string][]>(() => []),
  rootOf: vi.fn<(nodeId: number) => number>((nodeId) => nodeId),
  compareNodePosition: vi.fn<(nodeId: number, otherId: number) => number>(() => 0),
  isEqualNode: vi.fn<(nodeId: number, otherId: number) => boolean>(() => false),
  setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  getAttribute: vi.fn<(nodeId: number, key: string) => string | number | boolean | null>(
    () => null,
  ),
  removeAttribute: vi.fn<(nodeId: number, key: string) => void>(),
  setStyle: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  removeStyle: vi.fn<(nodeId: number, key: string) => void>(),
  inputValue: vi.fn<(nodeId: number) => string>(),
  setInputValue: vi.fn<(nodeId: number, value: string) => void>(),
  addEventListener: vi.fn<
    (
      nodeId: number,
      event: string,
      callback: HostCallback,
      capture?: boolean,
      passive?: boolean,
    ) => number
  >(() => nextCallbackId++),
  removeEventListener: vi.fn<(nodeId: number, event: string, callbackId: number) => boolean>(
    () => true,
  ),
  releaseListeners: vi.fn<(nodeId: number) => number[]>(() => []),
  templateContent: vi.fn<(nodeId: number) => number | null>(() => null),
  innerHtml: vi.fn<(nodeId: number) => string>(() => ""),
  outerHtml: vi.fn<(nodeId: number) => string>(() => ""),
  setInnerHtml: vi.fn<(nodeId: number, html: string) => Parsed>(() => [0, []]),
  setOuterHtml: vi.fn<(nodeId: number, html: string) => Parsed | null>(() => null),
  insertAdjacentHtml: vi.fn<(nodeId: number, position: string, html: string) => Parsed>(() => [
    0,
    [],
  ]),
  attributeNames: vi.fn<(nodeId: number) => string[]>(() => []),
  matches: vi.fn<(nodeId: number, selector: string) => boolean>(() => false),
  closest: vi.fn<(nodeId: number, selector: string) => number | null>(() => null),
  checkSelector: vi.fn<(selector: string) => void>(),
  querySelector: vi.fn<(nodeId: number, selector: string) => number | null>(() => null),
  querySelectorAll: vi.fn<(nodeId: number, selector: string) => number[]>(() => []),
  holdNode: vi.fn<(nodeId: number) => void>(),
  releaseNode: vi.fn<(nodeId: number) => void>(),
  focusNode: vi.fn<(nodeId: number) => void>(),
  blurNode: vi.fn<(nodeId: number) => void>(),
  focusedNode: vi.fn<() => number | null>(() => null),
  layoutOf: vi.fn<
    (nodeId: number) => readonly [number, number, number, number, number, number, number] | null
  >(() => null),
  scrollOf: vi.fn<(nodeId: number) => readonly [number, number, number, number] | null>(() => null),
  scrollNode: vi.fn<(nodeId: number, left: number | null, top: number | null) => void>(),
  scrollIntoViewNode: vi.fn<(nodeId: number, block: string, inline: string) => void>(),
  dispatchEvent: vi.fn<(nodeId: number, event: EventLike) => boolean>(() => true),
  click: vi.fn<(nodeId: number) => void>(),
  setPointerCapture: vi.fn<(nodeId: number, pointerId: number) => void>(),
  releasePointerCapture: vi.fn<(nodeId: number, pointerId: number) => void>(),
  hasPointerCapture: vi.fn<(nodeId: number, pointerId: number) => boolean>(),
  linkedNode: vi.fn<(nodeId: number, link: NodeLink) => number | null>(() => null),
  childNodeIds: vi.fn<(nodeId: number) => number[]>(() => []),
  isConnected: vi.fn<(nodeId: number) => boolean>(() => true),
  textContent: vi.fn<(nodeId: number) => string>(() => ""),
  wholeText: vi.fn<(nodeId: number) => string>(() => ""),
  substringData: vi.fn<(nodeId: number, offset: number, count: number) => string>(() => ""),
  replaceData: vi.fn<(nodeId: number, offset: number, count: number, data: string) => void>(),
  splitText: vi.fn<(nodeId: number, offset: number) => number>(() => 0),
};

beforeEach(() => {
  vi.clearAllMocks();
  nextCallbackId = 0;
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

  it("cloneNode", () => {
    native.cloneNode.mockReturnValueOnce([[7, "div"]]);
    expect(cloneNode(1, true)).toEqual([[7, "div"]]);
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
    native.setInnerHtml.mockReturnValueOnce([0, [[2, "b", 9, null]]]);
    expect(setInnerHtml(1, "<b>")).toEqual([0, [[2, "b", 9, null]]]);
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
    expect(native.linkedNode).toHaveBeenCalledWith(1, "next");
  });

  it("childNodeIds", () => {
    native.childNodeIds.mockReturnValueOnce([2, 3]);
    expect(childNodeIds(1)).toEqual([2, 3]);
    expect(native.childNodeIds).toHaveBeenCalledWith(1);
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

  it("getAttribute", () => {
    native.getAttribute.mockReturnValueOnce("hello");
    expect(getAttribute(1, "label")).toBe("hello");
    expect(native.getAttribute).toHaveBeenCalledWith(1, "label");
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

  it("layoutOf", () => {
    native.layoutOf.mockReturnValueOnce([1, 2, 3, 4, 0, 5, 6]);
    expect(layoutOf(1)).toEqual([1, 2, 3, 4, 0, 5, 6]);
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

  it("focusedNode", () => {
    native.focusedNode.mockReturnValueOnce(7);
    expect(focusedNode()).toBe(7);
  });
});
