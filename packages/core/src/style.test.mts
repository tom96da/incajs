// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { nativeStub } from "../tests/setup.mts";
import type { IncaNode } from "./element/types.mts";

interface FakeNode {
  tag: string;
  attrs: Map<string, unknown>;
  children: number[];
  parent: number | null;
}

const ROOT = 1;
let nodes: Map<number, FakeNode>;
let nextId: number;
let removeChildCalls: [number, number][];
let failNext: string | null;

/** Makes the next call of the named native throw once. */
function failOnce(name: string): void {
  failNext = name;
}

function guard(name: string): void {
  if (failNext !== name) return;
  failNext = null;
  throw new Error(`${name} failed`);
}

function makeNode(tag: string): number {
  const id = nextId++;
  nodes.set(id, { tag, attrs: new Map(), children: [], parent: null });
  return id;
}

function detach(id: number): void {
  const node = nodes.get(id)!;
  if (node.parent === null) return;
  const siblings = nodes.get(node.parent)!.children;
  siblings.splice(siblings.indexOf(id), 1);
  node.parent = null;
}

function attach(parent: number, child: number, before: number | null): number {
  detach(child);
  const siblings = nodes.get(parent)!.children;
  const at = before === null ? siblings.length : siblings.indexOf(before);
  siblings.splice(at, 0, child);
  nodes.get(child)!.parent = parent;
  return at;
}

function descendants(id: number): number[] {
  return nodes.get(id)!.children.flatMap((child) => [child, ...descendants(child)]);
}

function installNatives(): void {
  globalThis.__inca_native__ = nativeStub({
    rootNodeId: () => ROOT,
    createNode: (tag: string) => {
      guard("createNode");
      return makeNode(tag);
    },
    appendChild: (parent: number, child: number) => attach(parent, child, null),
    insertBefore: attach,
    removeChild: (parent: number, child: number) => {
      removeChildCalls.push([parent, child]);
      if (nodes.get(child)?.parent !== parent) {
        throw new DOMException("not a child", "NotFoundError");
      }
      detach(child);
    },
    setAttribute: (id: number, key: string, value: unknown) => nodes.get(id)!.attrs.set(key, value),
    getAttribute: (id: number, key: string) => nodes.get(id)!.attrs.get(key) ?? null,
    childNodeIds: (id: number) => [...nodes.get(id)!.children],
    linkedNode: (id: number) => nodes.get(id)!.parent,
    querySelectorAll: (id: number, selector: string) => {
      guard("querySelectorAll");
      expect(selector).toBe("style[data-inca-style]");
      return descendants(id).filter(
        (child) =>
          nodes.get(child)!.tag === "style" && nodes.get(child)!.attrs.has("data-inca-style"),
      );
    },
    replaceChild: (parent: number, child: number, old: number) => {
      const at = nodes.get(parent)!.children.indexOf(old);
      detach(old);
      attach(parent, child, nodes.get(parent)!.children[at] ?? null);
      return at;
    },
    cloneNode: (id: number, deep: boolean) => {
      const copy = (original: number): [number, string][] => {
        const own = nodes.get(original)!;
        const id = makeNode(own.tag);
        const below = deep ? own.children.flatMap(copy) : [];
        return [[id, own.tag], ...below];
      };
      return copy(id);
    },
    releaseListeners: () => [],
    holdNode: () => {},
    releaseNode: () => {},
  } as never);
}

/** The style elements below the root, as `[key, text]` pairs in tree order. */
function styles(): [unknown, string][] {
  return descendants(ROOT)
    .filter((id) => nodes.get(id)!.tag === "style")
    .map((id) => [
      nodes.get(id)!.attrs.get("data-inca-style"),
      nodes
        .get(id)!
        .children.map((child) => nodes.get(child)!.attrs.get("value"))
        .join(""),
    ]);
}

/** Builds a style element as an earlier evaluation of the app left it. */
function leftStyle(key: string, text: string, parent = ROOT): number {
  const id = makeNode("style");
  nodes.get(id)!.attrs.set("data-inca-style", key);
  const textId = makeNode("#text");
  nodes.get(textId)!.attrs.set("value", text);
  attach(id, textId, null);
  attach(parent, id, null);
  return id;
}

async function load() {
  vi.resetModules();
  return await import("./style.mts");
}

beforeEach(() => {
  nodes = new Map([[ROOT, { tag: "div", attrs: new Map(), children: [], parent: null }]]);
  nextId = 2;
  removeChildCalls = [];
  failNext = null;
  installNatives();
});

describe("updateStyle", () => {
  it("appends a style element with the text to the root container", async () => {
    const { updateStyle } = await load();

    updateStyle("a.css", ".a { width: 1px }");

    expect(styles()).toEqual([["a.css", ".a { width: 1px }"]]);
  });

  it("replaces the text of the element of the same key", async () => {
    const { updateStyle } = await load();
    updateStyle("a.css", "old");

    updateStyle("a.css", "new");

    expect(styles()).toEqual([["a.css", "new"]]);
  });

  it("keeps the place of an updated element", async () => {
    const { updateStyle } = await load();
    updateStyle("a.css", "a");
    updateStyle("b.css", "b");

    updateStyle("a.css", "a2");

    expect(styles()).toEqual([
      ["a.css", "a2"],
      ["b.css", "b"],
    ]);
  });

  it("appends in call order", async () => {
    const { updateStyle } = await load();

    updateStyle("a.css", "a");
    updateStyle("b.css", "b");

    expect(styles().map(([key]) => key)).toEqual(["a.css", "b.css"]);
  });

  it("leaves no text child for an empty text", async () => {
    const { updateStyle } = await load();

    updateStyle("a.css", "");

    expect(nodes.get(2)!.children).toEqual([]);
  });

  it("replaces the text of a found node that has an old text child", async () => {
    leftStyle("a.css", "old");
    const { updateStyle } = await load();

    updateStyle("a.css", "new");

    expect(styles()).toEqual([["a.css", "new"]]);
  });
});

/** The kind and tag of a handle and of every handle below it. */
function shape(node: IncaNode): unknown {
  const tag = node.kind === "element" ? node.tagName : node.kind;
  return node.kind === "text" ? tag : [tag, ...node.__children.map(shape)];
}

describe("the shadow tree of the root container", () => {
  it("holds the text of a created style element", async () => {
    const { updateStyle } = await load();
    const { rootElement } = await import("./element/handle.mts");
    updateStyle("a.css", "a");

    const copy = rootElement().cloneNode(true);

    expect(shape(copy)).toEqual(["DIV", ["STYLE", "text"]]);
  });

  it("holds a found style element and its text", async () => {
    leftStyle("a.css", "old");
    const { updateStyle } = await load();
    const { rootElement } = await import("./element/handle.mts");
    updateStyle("b.css", "b");

    const copy = rootElement().cloneNode(true);

    expect(shape(copy)).toEqual(["DIV", ["STYLE", "text"], ["STYLE", "text"]]);
  });

  it("puts an insertion after a found style element at its place", async () => {
    leftStyle("a.css", "a");
    const { updateStyle } = await load();
    const { rootElement, createElementNode } = await import("./element/handle.mts");
    updateStyle("b.css", "b");
    const root = rootElement();
    const div = createElementNode("div");

    root.insertBefore(div, root.__children[1]!);

    expect(root.__children.map((child) => child.__node_id)).toEqual(nodes.get(ROOT)!.children);
  });

  it("links found style elements in host order", async () => {
    const a = leftStyle("a.css", "a");
    const b = leftStyle("b.css", "b");
    const c = leftStyle("c.css", "c");
    const native = globalThis.__inca_native__ as {
      querySelectorAll: (id: number, selector: string) => number[];
    };
    const query = native.querySelectorAll;
    native.querySelectorAll = (id, selector) => query(id, selector).reverse();
    const { updateStyle } = await load();
    const { rootElement } = await import("./element/handle.mts");

    updateStyle("d.css", "d");

    expect(rootElement().__children.map((child) => child.__node_id)).toEqual([a, b, c, c + 2]);
  });

  it("puts a replacement after a found style element at its place", async () => {
    leftStyle("a.css", "a");
    const { updateStyle } = await load();
    const { rootElement, createElementNode } = await import("./element/handle.mts");
    updateStyle("b.css", "b");
    const root = rootElement();
    const div = createElementNode("div");

    root.replaceChild(div, root.__children[1]!);

    expect(root.__children.map((child) => child.__node_id)).toEqual(nodes.get(ROOT)!.children);
  });
});

describe("updateStyle after a failing native", () => {
  it("finds the existing element after a scan that threw", async () => {
    leftStyle("a.css", "old");
    const { updateStyle } = await load();
    failOnce("querySelectorAll");
    expect(() => updateStyle("a.css", "new")).toThrow("querySelectorAll failed");

    updateStyle("a.css", "new");

    expect(styles()).toEqual([["a.css", "new"]]);
  });

  it("keeps the old text when creating the new text node fails", async () => {
    leftStyle("a.css", "old");
    const { updateStyle } = await load();
    failOnce("createNode");

    expect(() => updateStyle("a.css", "new")).toThrow("createNode failed");

    expect(styles()).toEqual([["a.css", "old"]]);
  });
});

describe("removeStyle", () => {
  it("removes a created element through its handle", async () => {
    const { updateStyle, removeStyle } = await load();
    const { rootElement } = await import("./element/handle.mts");
    updateStyle("a.css", "a");
    const root = rootElement();

    removeStyle("a.css");

    expect(root.__children).toEqual([]);
    expect(styles()).toEqual([]);
  });

  it("removes the element of a key", async () => {
    const { updateStyle, removeStyle } = await load();
    updateStyle("a.css", "a");
    updateStyle("b.css", "b");

    removeStyle("a.css");

    expect(styles()).toEqual([["b.css", "b"]]);
  });

  it("creates a fresh element after a removal", async () => {
    const { updateStyle, removeStyle } = await load();
    updateStyle("a.css", "a");
    removeStyle("a.css");

    updateStyle("a.css", "again");

    expect(styles()).toEqual([["a.css", "again"]]);
  });

  it("ignores an unknown key", async () => {
    const { removeStyle } = await load();

    expect(() => removeStyle("none.css")).not.toThrow();
    expect(removeChildCalls).toEqual([]);
  });

  it("removes a found element through its own parent", async () => {
    const wrapper = makeNode("div");
    attach(ROOT, wrapper, null);
    const id = leftStyle("a.css", "a", wrapper);
    const { removeStyle } = await load();

    removeStyle("a.css");

    expect(removeChildCalls).toEqual([[wrapper, id]]);
  });

  it("skips a found element that the app detached after the scan", async () => {
    const id = leftStyle("a.css", "a");
    const { removeStyle, updateStyle } = await load();
    updateStyle("scan.css", "x");
    detach(id);
    removeChildCalls = [];

    expect(() => removeStyle("a.css")).not.toThrow();
    expect(removeChildCalls).toEqual([]);
  });
});
