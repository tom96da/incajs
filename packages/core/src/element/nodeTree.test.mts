// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { document } from "../document.mts";
import { createCommentNode, createElementNode, createIncaElement, rootElement } from "./handle.mts";
import { markDestroyed } from "./lifecycle.mts";
import { createTextNode } from "./shadowTree.mts";
import type { NodeLink } from "../native.mts";

/** The node each node reaches by each link. */
let links: Record<number, Partial<Record<NodeLink, number>>> = {};
/** The children of each node. */
let children: Record<number, number[]> = {};
/** The data of each text node or comment stand-in. */
let data: Record<number, string> = {};
let nextId = 100;

const host = {
  rootNodeId: vi.fn<() => number>(() => 0),
  createNode: vi.fn<(tag: string) => number>(() => nextId++),
  setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  getAttribute: vi.fn<(nodeId: number, key: string) => string | null>(
    (nodeId) => data[nodeId] ?? null,
  ),
  setStyle: vi.fn<() => void>(),
  linkedNode: vi.fn<(nodeId: number, link: NodeLink) => number | null>(
    (nodeId, link) => links[nodeId]?.[link] ?? null,
  ),
  childNodeIds: vi.fn<(nodeId: number) => number[]>((nodeId) => children[nodeId] ?? []),
  isConnected: vi.fn<(nodeId: number) => boolean>(() => true),
};

beforeEach(() => {
  vi.clearAllMocks();
  links = {};
  children = {};
  data = {};
  nextId = 100;
  globalThis.__inca_native__ = host as never;
});

describe("nodeType", () => {
  it.each([
    ["an element", () => createElementNode("button"), 1],
    ["a text node", () => createTextNode("t"), 3],
    ["a comment stand-in", () => createCommentNode(), 8],
  ] as const)("reads %s", (_, make, type) => {
    expect(make().nodeType).toBe(type);
  });
});

describe("nodeName", () => {
  it.each([
    ["an element", () => createElementNode("button"), "BUTTON"],
    ["a text node", () => createTextNode("t"), "#text"],
    ["a comment stand-in", () => createCommentNode(), "#comment"],
  ] as const)("reads %s", (_, make, name) => {
    expect(make().nodeName).toBe(name);
  });
});

describe("nodeValue", () => {
  it("is null for an element", () => {
    expect(createElementNode("div").nodeValue).toBeNull();
  });

  it("stays null after an assignment to an element", () => {
    const el = createElementNode("div");

    el.nodeValue = "x";

    expect(el.nodeValue).toBeNull();
  });

  it("sends nothing to the host for an assignment to an element", () => {
    createElementNode("div").nodeValue = "x";

    expect(host.setAttribute).not.toHaveBeenCalled();
  });

  it.each([
    ["a text node", () => createTextNode("first")],
    ["a comment stand-in", () => createCommentNode()],
  ] as const)("reads the data of %s from the host", (_, make) => {
    const node = make();
    data[node.__node_id] = "second";

    expect(node.nodeValue).toBe("second");
  });

  it.each([
    [null, ""],
    [undefined, ""],
    ["x", "x"],
  ] as const)("sends %s to the host as %j for a text node", (value, stored) => {
    const text = createTextNode("first");
    host.setAttribute.mockClear();

    text.nodeValue = value as string | null;

    expect(host.setAttribute).toHaveBeenCalledWith(text.__node_id, "value", stored);
  });

  it.each([
    [null, ""],
    [undefined, ""],
    ["x", "x"],
  ] as const)("mirrors %s as %j in the text field", (value, stored) => {
    const text = createTextNode("first");

    text.nodeValue = value as string | null;

    expect(text.text).toBe(stored);
  });

  it("sends the text of a new comment stand-in to the host", () => {
    const comment = createCommentNode("x");

    expect(host.setAttribute).toHaveBeenCalledWith(comment.__node_id, "value", "x");
  });

  it("sends an assignment to a comment stand-in to the host", () => {
    const comment = createCommentNode();

    comment.nodeValue = "note";

    expect(host.setAttribute).toHaveBeenCalledWith(comment.__node_id, "value", "note");
  });
});

describe("isConnected", () => {
  it.each([true, false])("reads %s from the host", (connected) => {
    host.isConnected.mockReturnValueOnce(connected);

    expect(createElementNode("div").isConnected).toBe(connected);
  });

  it("asks the host about its own node", () => {
    const el = createElementNode("div");

    void el.isConnected;

    expect(host.isConnected).toHaveBeenCalledWith(el.__node_id);
  });
});

describe("ownerDocument", () => {
  it.each([
    ["element", () => createElementNode("div")],
    ["text node", () => createTextNode("t")],
  ] as const)("is the document of a detached %s", (_, make) => {
    expect(make().ownerDocument).toBe(document);
  });
});

describe("parentNode", () => {
  it("is the handle of the parent", () => {
    const parent = createIncaElement(1, "element", "div");
    const child = createIncaElement(2, "element", "div");
    links[2] = { parent: 1 };

    expect(child.parentNode).toBe(parent);
  });

  it("is null for a detached node", () => {
    expect(createElementNode("div").parentNode).toBeNull();
  });

  it("is the document for the mount container", () => {
    expect(rootElement().parentNode).toBe(document);
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
  ] as const)("%s is null when the %s node has no live handle", (member, link) => {
    const node = createElementNode("div");
    links[node.__node_id] = { [link]: 999 };

    expect(node[member]).toBeNull();
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

    children[parent.__node_id] = [createTextNode("t").__node_id];

    expect(list.length).toBe(1);
  });

  it("lists the handle of a later insert", () => {
    const parent = createElementNode("div");
    const list = parent.childNodes;
    const text = createTextNode("t");

    children[parent.__node_id] = [text.__node_id];

    expect(list[0]).toBe(text);
  });

  it("skips a child without a live handle", () => {
    const parent = createElementNode("div");
    const text = createTextNode("t");
    children[parent.__node_id] = [999, text.__node_id];

    expect([...parent.childNodes]).toEqual([text]);
  });

  it("is empty for a text node", () => {
    expect(createTextNode("t").childNodes.length).toBe(0);
  });

  it.each([
    ["no child", () => [], false],
    ["a live child", () => [createTextNode("t").__node_id], true],
    ["a child without a live handle", () => [999], false],
  ] as const)("hasChildNodes reads %s", (_, makeIds, expected) => {
    const el = createElementNode("div");
    children[el.__node_id] = makeIds();

    expect(el.hasChildNodes()).toBe(expected);
  });
});

describe("a freed node", () => {
  /** A freed element whose host answers every link, with a child list it read before. */
  function freed() {
    const parent = createElementNode("div");
    const child = createElementNode("div");
    const id = child.__node_id;
    links[id] = { parent: parent.__node_id, first: 1, last: 1, previous: 1, next: 1 };
    children[id] = [parent.__node_id];
    const list = child.childNodes;
    markDestroyed(child);
    vi.clearAllMocks();
    return { child, list };
  }

  it.each(["parentNode", "firstChild", "lastChild", "previousSibling", "nextSibling"] as const)(
    "reads null for %s",
    (member) => {
      expect(freed().child[member]).toBeNull();
    },
  );

  it("reads an empty childNodes", () => {
    expect(freed().list.length).toBe(0);
  });

  it("reads false for hasChildNodes", () => {
    expect(freed().child.hasChildNodes()).toBe(false);
  });

  it("reads false for isConnected", () => {
    expect(freed().child.isConnected).toBe(false);
  });

  it("asks the host for no link", () => {
    const { child } = freed();

    void [child.parentNode, child.firstChild, child.nextSibling];

    expect(host.linkedNode).not.toHaveBeenCalled();
  });

  it("asks the host nothing for isConnected", () => {
    void freed().child.isConnected;

    expect(host.isConnected).not.toHaveBeenCalled();
  });

  it("keeps the last data of a text node", () => {
    const text = createTextNode("kept");
    markDestroyed(text);

    expect(text.nodeValue).toBe("kept");
  });

  it("has an empty nodeValue as a comment stand-in", () => {
    const comment = createCommentNode();
    markDestroyed(comment);

    expect(comment.nodeValue).toBe("");
  });

  it("ignores an assignment to nodeValue", () => {
    const text = createTextNode("kept");
    markDestroyed(text);
    host.setAttribute.mockClear();

    text.nodeValue = "changed";

    expect(host.setAttribute).not.toHaveBeenCalled();
  });
});
