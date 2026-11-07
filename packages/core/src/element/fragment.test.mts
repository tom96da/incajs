// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { document } from "../document.mts";
import "./fragment.mts";
import { createElementNode } from "./handle.mts";
import { isHeld, markDestroyed } from "./lifecycle.mts";
import type { NodeLink } from "../native.mts";

let nextId = 1;
/** The parent each node reaches through `linkedNode`. */
let parents = new Map<number, number>();

const host = {
  rootNodeId: vi.fn<() => number>(() => 0),
  createNode: vi.fn<(tag: string) => number>(() => nextId++),
  setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  appendChild: vi.fn<(parentId: number, childId: number) => number>(() => 0),
  removeChild: vi.fn<(parentId: number, childId: number) => void>(),
  releaseListeners: vi.fn<(nodeId: number) => number[]>(() => []),
  holdNode: vi.fn<(nodeId: number) => void>(),
  releaseNode: vi.fn<(nodeId: number) => void>(),
  layoutOf: vi.fn<() => readonly number[]>(() => [0, 0, 10, 10, 0, 0, 0]),
  isConnected: vi.fn<(nodeId: number) => boolean>(() => false),
  textContent: vi.fn<(nodeId: number) => string>(() => "joined"),
  linkedNode: vi.fn<(nodeId: number, link: NodeLink) => number | null>((nodeId, link) =>
    link === "parent" ? (parents.get(nodeId) ?? null) : null,
  ),
};

beforeEach(() => {
  vi.clearAllMocks();
  nextId = 1;
  parents = new Map();
  globalThis.__inca_native__ = host as never;
});

describe("document.createDocumentFragment", () => {
  it("allocates a native fragment node", () => {
    document.createDocumentFragment();

    expect(host.createNode).toHaveBeenCalledWith("#fragment");
  });

  it("returns an empty, parentless fragment", () => {
    const fragment = document.createDocumentFragment();

    expect(fragment).toMatchObject({ kind: "fragment", parent: null, __children: [] });
  });

  it("holds the node until its handle is collected", () => {
    const fragment = document.createDocumentFragment();

    expect(host.holdNode).toHaveBeenCalledWith(fragment.__node_id);
    expect(isHeld(fragment)).toBe(true);
  });

  it.each([
    ["nodeType", 11],
    ["nodeName", "#document-fragment"],
    ["nodeValue", null],
    ["isConnected", false],
    ["ownerDocument", document],
  ] as const)("reads %s of a fragment", (member, value) => {
    expect(document.createDocumentFragment()[member]).toBe(value);
  });

  it("ignores an assignment to nodeValue", () => {
    const fragment = document.createDocumentFragment();

    fragment.nodeValue = "x";

    expect(fragment.nodeValue).toBeNull();
    expect(host.setAttribute).not.toHaveBeenCalled();
  });

  it.each([
    "addEventListener",
    "removeEventListener",
    "dispatchEvent",
    "querySelector",
    "querySelectorAll",
    "children",
    "getElementById",
  ])("has no %s", (member) => {
    expect(member in document.createDocumentFragment()).toBe(false);
  });

  it("reads textContent from the host", () => {
    expect(document.createDocumentFragment().textContent).toBe("joined");
  });

  it("reads an empty textContent once the host freed it", () => {
    const fragment = document.createDocumentFragment();
    markDestroyed(fragment);

    expect(fragment.textContent).toBe("");
  });

  it("replaces its children with one text node when textContent is assigned", () => {
    const fragment = document.createDocumentFragment();
    fragment.appendChild(createElementNode("div"));

    fragment.textContent = "x";

    expect(fragment.__children.map((child) => child.kind)).toEqual(["text"]);
  });

  it("ignores a textContent assignment once the host freed it", () => {
    const fragment = document.createDocumentFragment();
    markDestroyed(fragment);

    fragment.textContent = "x";

    expect(host.setAttribute).not.toHaveBeenCalled();
  });
});

describe("a node inside a fragment", () => {
  const inside = () => {
    const fragment = document.createDocumentFragment();
    const child = createElementNode("div");
    fragment.appendChild(child);
    parents.set(child.__node_id, fragment.__node_id);
    return { fragment, child };
  };

  it("reads the fragment as parentNode", () => {
    const { fragment, child } = inside();

    expect(child.parentNode).toBe(fragment);
  });

  it("has no offsetParent", () => {
    expect(inside().child.offsetParent).toBeNull();
  });

  it("reads null as parentElement", () => {
    expect(inside().child.parentElement).toBeNull();
  });

  it("reads the fragment from getRootNode", () => {
    const { fragment, child } = inside();

    expect(child.getRootNode()).toBe(fragment);
  });
});
