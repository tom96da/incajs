// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { adoptElement } from "../../tests/adopt.mts";
import { keep } from "../../tests/keep.mts";
import { nativeStub } from "../../tests/setup.mts";
import { document } from "../document.mts";
import { createCommentNode, createElementNode, createTextNode, rootElement } from "./handle.mts";
import { markFreed } from "./lifecycle.mts";
import type { NodeLink } from "../ffi/index.mts";

/** The id `nativeStub` gives the document. */
const DOCUMENT = 1;
/** The node each node reaches by each link. */
let links: Record<number, Partial<Record<NodeLink, number>>> = {};
/** The children of each node. */
let children: Record<number, number[]> = {};
/** The data of each text node or comment. */
let data: Record<number, string> = {};
let nextId = 100;

const host = {
  rootNodeId: vi.fn<() => number>(() => 0),
  createNode: vi.fn<(tag: string) => number>(() => nextId++),
  setCharacterData: vi.fn<(nodeId: number, data: string) => void>(),
  textContent: vi.fn<(nodeId: number) => string>((nodeId) => data[nodeId] ?? ""),
  setStyle: vi.fn<() => void>(),
  linkedNode: vi.fn<(nodeId: number, link: NodeLink) => number | null>(
    (nodeId, link) => links[nodeId]?.[link] ?? null,
  ),
  childNodeIds: vi.fn<(nodeId: number) => number[]>((nodeId) => children[nodeId] ?? []),
  childCount: vi.fn<(nodeId: number, elementOnly: boolean) => number>(
    (nodeId) => (children[nodeId] ?? []).length,
  ),
  childAt: vi.fn<(nodeId: number, index: number, elementOnly: boolean) => number | null>(
    (nodeId, index) => children[nodeId]?.[index] ?? null,
  ),
  isConnected: vi.fn<(nodeId: number) => boolean>(() => true),
};

beforeEach(() => {
  vi.clearAllMocks();
  links = {};
  children = {};
  data = {};
  nextId = 100;
  vi.stubGlobal("__inca_native__", nativeStub(host as never));
});

describe("ownerDocument", () => {
  it.each([
    ["element", () => createElementNode("div")],
    ["text node", () => createTextNode("t")],
  ] as const)("is the document of a detached %s", (_, make) => {
    expect(make().ownerDocument).toBe(document);
  });

  it("is the document of a freed element", () => {
    const el = createElementNode("div");
    markFreed([el.__node_id]);

    expect(el.ownerDocument).toBe(document);
  });
});

describe("parentNode", () => {
  it("is the handle of the parent", () => {
    const parent = adoptElement(11, "div");
    const child = adoptElement(12, "div");
    links[12] = { parent: 11 };

    expect(child.parentNode).toBe(parent);
  });

  it("is null for a detached node", () => {
    expect(createElementNode("div").parentNode).toBeNull();
  });

  it("is the document for the mount container", () => {
    const root = rootElement();
    links[root.__node_id] = { parent: DOCUMENT };

    expect(root.parentNode).toBe(document);
  });
});

describe("parentElement", () => {
  it("reads the parent element of a text node through the host", () => {
    const parent = createElementNode("div");
    const text = createTextNode("t");
    links[text.__node_id] = { parent: parent.__node_id };

    expect(text.parentElement).toBe(parent);
    expect(host.linkedNode).toHaveBeenCalledWith(text.__node_id, "parent", true);
  });
});

describe("links", () => {
  it.each(["firstChild", "lastChild", "previousSibling", "nextSibling"] as const)(
    "%s is null without a node to reach",
    (member) => {
      expect(createElementNode("div")[member]).toBeNull();
    },
  );

  it.each([
    ["firstChild", "first"],
    ["lastChild", "last"],
    ["previousSibling", "previous"],
    ["nextSibling", "next"],
    ["parentNode", "parent"],
  ] as const)("%s builds a handle when the %s node has none", (member, link) => {
    const node = createElementNode("div");
    links[node.__node_id] = { [link]: 999 };

    expect(node[member]?.__node_id).toBe(999);
  });

  it("firstChild is the registered handle of a text node", () => {
    const parent = createElementNode("div");
    const text = createTextNode("t");
    links[parent.__node_id] = { first: text.__node_id };

    expect(parent.firstChild).toBe(text);
  });

  it.each([
    ["lastChild", "last"],
    ["previousSibling", "previous"],
    ["nextSibling", "next"],
  ] as const)("%s asks the host for the %s link", (member, link) => {
    const node = createElementNode("div");
    const other = createCommentNode();
    links[node.__node_id] = { [link]: other.__node_id };

    expect(node[member]).toBe(other);
  });
});

describe("childNodes", () => {
  it("is the same object on two reads", () => {
    const el = createElementNode("div");

    expect(el.childNodes).toBe(el.childNodes);
  });

  it("is empty before an insert", () => {
    expect(createElementNode("div").childNodes.length).toBe(0);
  });

  it("counts a later insert", () => {
    const parent = createElementNode("div");
    const list = parent.childNodes;

    children[parent.__node_id] = [keep(createTextNode("t")).__node_id];

    expect(list.length).toBe(1);
  });

  it("lists the handle of a later insert", () => {
    const parent = createElementNode("div");
    const list = parent.childNodes;
    const text = createTextNode("t");

    children[parent.__node_id] = [text.__node_id];

    expect(list[0]).toBe(text);
  });

  it("builds a handle for a child that has none", () => {
    const parent = createElementNode("div");
    const text = createTextNode("t");
    children[parent.__node_id] = [999, text.__node_id];

    expect([...parent.childNodes].map((node) => node.__node_id)).toEqual([999, text.__node_id]);
  });

  it("reads every child, not only the elements", () => {
    const parent = createElementNode("div");
    children[parent.__node_id] = [keep(createTextNode("t")).__node_id];

    void [parent.childNodes.length, parent.childNodes[0]];

    expect([host.childCount.mock.calls[0]![1], host.childAt.mock.calls[0]![2]]).toEqual([
      false,
      false,
    ]);
  });

  it("is empty for a text node", () => {
    expect(createTextNode("t").childNodes.length).toBe(0);
  });

  it.each([
    ["no child", () => [], false],
    ["a live child", () => [keep(createTextNode("t")).__node_id], true],
    ["a child without a live handle", () => [999], true],
  ] as const)("hasChildNodes reads %s", (_, makeIds, expected) => {
    const el = createElementNode("div");
    children[el.__node_id] = makeIds();

    expect(el.hasChildNodes()).toBe(expected);
  });
});

describe("a freed node", () => {
  /** A freed element whose host answers every link. */
  function freed() {
    const parent = createElementNode("div");
    const child = createElementNode("div");
    const id = child.__node_id;
    links[id] = { parent: parent.__node_id, first: 1, last: 1, previous: 1, next: 1 };
    markFreed([child.__node_id]);
    vi.clearAllMocks();
    return { child };
  }

  it.each(["parentNode", "firstChild", "lastChild", "previousSibling", "nextSibling"] as const)(
    "reads null for %s",
    (member) => {
      expect(freed().child[member]).toBeNull();
    },
  );

  it("asks the host for no link", () => {
    const { child } = freed();

    void [child.parentNode, child.firstChild, child.nextSibling];

    expect(host.linkedNode).not.toHaveBeenCalled();
  });
});

describe("a second evaluation of core", () => {
  it("is the document when no handle of it was built yet", async () => {
    vi.resetModules();
    const fresh = await import("./handle.mts");
    const child = fresh.adoptHandle(12);
    links[12] = { parent: DOCUMENT };

    expect(child.parentNode).toBe(fresh.documentHandle());
  });
});
