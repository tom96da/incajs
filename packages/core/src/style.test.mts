// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { installFakeHost } from "../tests/fakeHost.mts";
import { collectGarbage, collectSentinel } from "../tests/gc.mts";
import type { FakeHost } from "../tests/fakeHost.mts";
import type { HandleNode } from "./element/handleNode.mts";
import type { Node } from "./element/nodeTypes.mts";
import type { CSSStyleDeclaration, Element, Text } from "./element/types.mts";

let fake: FakeHost;
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

function descendants(id: number): number[] {
  return fake.nodes.get(id)!.children.flatMap((child) => [child, ...descendants(child)]);
}

function installNatives(): void {
  fake = installFakeHost({
    styleOf: () => ({}) as CSSStyleDeclaration,
    createNode: (tag: string) => {
      guard("createNode");
      return fake.create(tag);
    },
    querySelectorAll: (id: number, selector: string) => {
      guard("querySelectorAll");
      expect(selector).toBe("style[data-inca-style]");
      return descendants(id).filter(
        (child) =>
          fake.nodes.get(child)!.tag === "style" &&
          fake.nodes.get(child)!.attrs.has("data-inca-style"),
      );
    },
  });
}

/** The style elements below the root, as `[key, text]` pairs in tree order. */
function styles(): [unknown, string][] {
  return descendants(fake.ROOT)
    .filter((id) => fake.nodes.get(id)!.tag === "style")
    .map((id) => [
      fake.nodes.get(id)!.attrs.get("data-inca-style"),
      fake.nodes
        .get(id)!
        .children.map((child) => fake.nodes.get(child)!.attrs.get("value"))
        .join(""),
    ]);
}

/** Builds a style element as an earlier evaluation of the app left it. */
function leftStyle(key: string, text: string, parent = fake.ROOT): number {
  const id = fake.create("style");
  fake.nodes.get(id)!.attrs.set("data-inca-style", key);
  const textId = fake.create("#text");
  fake.natives.setAttribute!(textId, "value", text);
  fake.natives.appendChild!(id, textId);
  fake.natives.appendChild!(parent, id);
  return id;
}

async function load() {
  vi.resetModules();
  return await import("./style.mts");
}

beforeEach(() => {
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

    expect(fake.nodes.get(fake.idsOf("style")[0]!)!.children).toEqual([]);
  });

  it("replaces the text of a found node that has an old text child", async () => {
    leftStyle("a.css", "old");
    const { updateStyle } = await load();

    updateStyle("a.css", "new");

    expect(styles()).toEqual([["a.css", "new"]]);
  });
});

/** The kind and tag of a handle and of every handle below it. */
function shape(value: Node): unknown {
  const node = value as HandleNode;
  const tag = node.kind === "element" ? node.tagName : node.kind;
  return node.kind === "text" ? tag : [tag, ...Array.from(node.childNodes).map(shape)];
}

describe("the tree of the root container", () => {
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

  it("reads STYLE as the tag name of a found style element", async () => {
    leftStyle("a.css", "old");
    const { updateStyle } = await load();
    const { rootElement } = await import("./element/handle.mts");
    updateStyle("b.css", "b");

    expect((rootElement().firstChild as Element).tagName).toBe("STYLE");
  });

  it("reaches the text children of a found style element", async () => {
    leftStyle("a.css", "old");
    const { updateStyle } = await load();
    const { rootElement } = await import("./element/handle.mts");
    updateStyle("b.css", "b");
    await collectSentinel();

    const found = rootElement().firstChild as Element;

    expect((found.firstChild as Text).text).toBe("old");
  });
});

describe("the text nodes of an updated style element", () => {
  it("lets the old text node be collected", async () => {
    const { updateStyle } = await load();
    const { rootElement } = await import("./element/handle.mts");
    updateStyle("a.css", "old");
    const old = new WeakRef(rootElement().firstChild!.firstChild!);

    updateStyle("a.css", "new");
    await collectGarbage(() => old.deref() === undefined);

    expect(old.deref()).toBeUndefined();
  });

  it("keeps the new text node after a collection", async () => {
    const { updateStyle } = await load();
    const { nodeOf } = await import("./element/lifecycle.mts");
    updateStyle("a.css", "old");
    updateStyle("a.css", "new");
    await collectSentinel();

    const [text] = fake.nodes.get(fake.idsOf("style")[0]!)!.children;

    expect(nodeOf(text!)).toBeDefined();
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

    expect(root.childNodes.length).toBe(0);
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
    expect(fake.spies.removeNode).not.toHaveBeenCalled();
  });

  it("removes a found element from its own parent", async () => {
    const wrapper = fake.create("div");
    fake.natives.appendChild!(fake.ROOT, wrapper);
    const id = leftStyle("a.css", "a", wrapper);
    const { removeStyle } = await load();

    removeStyle("a.css");

    expect(fake.nodes.get(wrapper)!.children).not.toContain(id);
  });

  it("makes one removeNode call", async () => {
    const { updateStyle, removeStyle } = await load();
    updateStyle("a.css", "a");
    const id = fake.idsOf("style")[0]!;

    removeStyle("a.css");

    expect(fake.spies.removeNode.mock.calls).toEqual([[id]]);
  });

  it("reads the removed element as removed", async () => {
    const { updateStyle, removeStyle } = await load();
    const { rootElement } = await import("./element/handle.mts");
    const { isRemoved } = await import("./element/lifecycle.mts");
    updateStyle("a.css", "a");
    const el = rootElement().firstChild!;

    removeStyle("a.css");

    expect(isRemoved(el)).toBe(true);
  });

  it("removes a found element that the app took out of the tree after the scan", async () => {
    const id = leftStyle("a.css", "a");
    const { removeStyle, updateStyle } = await load();
    updateStyle("scan.css", "x");
    fake.natives.removeChild!(fake.ROOT, id);

    expect(() => removeStyle("a.css")).not.toThrow();
  });

  it("lets the handle of the removed element be collected", async () => {
    const { updateStyle, removeStyle } = await load();
    const { rootElement } = await import("./element/handle.mts");
    updateStyle("a.css", "a");
    const handle = new WeakRef(rootElement().firstChild!);

    removeStyle("a.css");
    await collectGarbage(() => handle.deref() === undefined);

    expect(handle.deref()).toBeUndefined();
  });

  it("lets the host free the removed node once its handle is collected", async () => {
    const { updateStyle, removeStyle } = await load();
    updateStyle("a.css", "a");
    const id = fake.idsOf("style")[0]!;

    removeStyle("a.css");
    await collectGarbage(() => !fake.held.has(id));
    fake.frame();

    expect(fake.nodes.has(id)).toBe(false);
  });
});
