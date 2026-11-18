// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { installFakeHost } from "../../tests/fakeHost.mts";
import { collectGarbage, collectSentinel } from "../../tests/gc.mts";
import { keep } from "../../tests/keep.mts";
import { nativeStub } from "../../tests/setup.mts";
import { document } from "../document.mts";
import { createElementNode, createTextNode, setElementText } from "./handle.mts";
import { elementOf, markFreed, nodeOf } from "./lifecycle.mts";
import { setText } from "./text.mts";
import { insert } from "./treeOps.mts";
import type { FakeHost } from "../../tests/fakeHost.mts";
import type { Node } from "./nodeTypes.mts";
import type { Text } from "./types.mts";

type Native = typeof globalThis.__inca_native__;

let fake: FakeHost;

/** The data of a text node or comment in the host. */
const dataOf = (node: Node): string | null => fake.nodes.get(node.__node_id)!.data;

/** The children of `node` as the host lists them. */
const kids = (node: Node): Node[] => Array.from(node.childNodes);

const host = {
  createNode: vi.fn<(tag: string) => number>(),
  setAttribute: vi.fn<(nodeId: number, key: string, value: string | number | boolean) => void>(),
  getAttribute: vi.fn<(nodeId: number, key: string) => string | null>(),
  textContent: vi.fn<(nodeId: number) => string>(() => ""),
  setCharacterData: vi.fn<(nodeId: number, data: string) => void>(),
  splitText: vi.fn<(nodeId: number, offset: number) => number>(),
  replaceWithText: vi.fn<Native["replaceWithText"]>(),
};

beforeEach(() => {
  vi.clearAllMocks();
  fake = installFakeHost();
  host.createNode.mockImplementation(fake.natives.createNode!);
  host.setAttribute.mockImplementation(fake.natives.setAttribute!);
  host.getAttribute.mockImplementation(fake.natives.getAttribute! as never);
  host.textContent.mockImplementation(fake.natives.textContent!);
  host.setCharacterData.mockImplementation(fake.natives.setCharacterData!);
  host.splitText.mockImplementation(fake.natives.splitText!);
  host.replaceWithText.mockImplementation(fake.natives.replaceWithText!);
  vi.stubGlobal("__inca_native__", nativeStub({ ...fake.natives, ...host } as never));
});

describe("textContent of a text node", () => {
  it.each([
    [null, ""],
    [undefined, ""],
    ["x", "x"],
  ] as const)("stores %s as %j", (value, stored) => {
    const text = createTextNode("first");

    text.textContent = value as string;

    expect(dataOf(text)).toBe(stored);
  });
});

describe("splitText", () => {
  it("returns the node the host made, holding the tail", () => {
    const text = createTextNode("abcd");
    host.splitText.mockImplementationOnce((id, offset) => {
      const tailId = fake.natives.splitText!(id, offset);
      fake.nodes.get(tailId)!.data = "cd";
      return tailId;
    });

    const tail = text.splitText(2);

    expect(tail.__node_id).toBe(host.splitText.mock.results[0]!.value);
    expect(tail.textContent).toBe("cd");
  });

  it("places the tail after the node in its parent", () => {
    const parent = createElementNode("div");
    const text = createTextNode("abcd");
    const last = createTextNode("z");
    insert(text, parent);
    insert(last, parent);

    const tail = text.splitText(2);

    expect(kids(parent).map((node) => node.__node_id)).toEqual(
      [text, tail, last].map((node) => node.__node_id),
    );
    expect(tail.parentNode).toBe(parent);
  });

  it("leaves the tail with no parent for a node with no parent", () => {
    expect(createTextNode("abcd").splitText(2).parentNode).toBeNull();
  });

  it("gives the tail the document as ownerDocument", () => {
    expect(createTextNode("abcd").splitText(2).ownerDocument).toBe(document);
  });

  it("throws a TypeError for a freed node", () => {
    const text = createTextNode("abcd");
    markFreed([text.__node_id]);

    expect(() => text.splitText(1)).toThrow(TypeError);
  });
});

describe("textContent of an element", () => {
  it("replaces the children with one text node", () => {
    const el = createElementNode("div");
    insert(createTextNode("old"), el);

    el.textContent = "new";

    expect(kids(el).map((child) => child.textContent)).toEqual(["new"]);
  });

  it.each([null, undefined, ""] as const)("removes the children for %j", (value) => {
    const el = createElementNode("div");
    insert(createTextNode("old"), el);

    el.textContent = value as string;

    expect(kids(el).map((node) => node.__node_id)).toEqual([]);
  });

  it("keeps the listeners of a removed child", () => {
    const el = createElementNode("div");
    const child = createElementNode("span");
    insert(child, el);

    el.textContent = "new";

    expect(fake.spies.removeNode).not.toHaveBeenCalled();
  });

  it("ignores an assignment to a freed element", () => {
    const el = createElementNode("div");
    insert(createTextNode("old"), el);
    markFreed([el.__node_id]);

    el.textContent = "new";

    expect(fake.nodes.get(el.__node_id)!.children).toHaveLength(1);
  });
});

describe("createText", () => {
  it("creates a text node and forwards the initial value", () => {
    const node = createTextNode("hello");
    expect(host.createNode).toHaveBeenCalledWith("#text");
    expect(host.setCharacterData).toHaveBeenCalledWith(node.__node_id, "hello");
    expect(node).toMatchObject({ kind: "text", textContent: "hello" });
    expect("id" in node).toBe(false);
  });
});

describe("setText", () => {
  it("forwards the new value and updates the node", () => {
    const node = createTextNode("old") as Text;
    setText(node, "new");
    expect(host.setCharacterData).toHaveBeenCalledWith(node.__node_id, "new");
    expect(node.textContent).toBe("new");
  });
});

describe("setElementText", () => {
  it("replaces existing children with one text child", () => {
    const el = createElementNode("div");
    const oldChild = createElementNode("div");
    insert(oldChild, el);

    setElementText(el, "hello");

    expect(host.replaceWithText).toHaveBeenCalledWith(el.__node_id, "hello");
    expect(oldChild.parentNode).toBeNull();
    expect(kids(el)).toHaveLength(1);
    expect(kids(el)[0]).toMatchObject({ kind: "text", textContent: "hello" });
  });

  it.each<[string, (el: Node) => void]>([
    [
      "an assignment to textContent",
      (el) => {
        el.textContent = "\ud800";
      },
    ],
    ["setElementText", (el) => setElementText(el as never, "\ud800")],
  ])("keeps the children when %s cannot create its text node", (_name, run) => {
    const el = createElementNode("div");
    const oldChild = createElementNode("div");
    insert(oldChild, el);
    host.replaceWithText.mockImplementationOnce(() => {
      throw new TypeError("the text has a lone surrogate");
    });

    expect(() => run(el)).toThrow(TypeError);
    expect(oldChild.parentNode).toBe(el);
  });

  it("leaves no children behind for an empty string", () => {
    const el = createElementNode("div");
    const oldChild = createElementNode("div");
    insert(oldChild, el);

    setElementText(el, "");

    expect(kids(el)).toHaveLength(0);
  });
});

describe("the handles below a child textContent replaces", () => {
  it("lives while the handle of the child lives", async () => {
    const el = createElementNode("div");
    const top = keep(createElementNode("div"));
    const inner = (() => {
      const child = createElementNode("div");
      insert(top, el);
      insert(child, top);
      return new WeakRef(child);
    })();

    el.textContent = "x";
    await collectSentinel();

    expect(inner.deref()).toBeDefined();
  });

  it("is collected with the handle of the child", async () => {
    const el = createElementNode("div");
    const inner = (() => {
      const top = createElementNode("div");
      const child = createElementNode("div");
      insert(top, el);
      insert(child, top);
      el.textContent = "x";
      return new WeakRef(child);
    })();

    await collectGarbage(() => inner.deref() === undefined);

    expect(inner.deref()).toBeUndefined();
  });
});

describe("remove", () => {
  it("keeps the children setElementText replaces readable", () => {
    const root = createElementNode("div");
    const child = createElementNode("div");
    insert(child, root);
    vi.mocked(host.getAttribute).mockReturnValueOnce("c");

    setElementText(root, "");

    expect(child.id).toBe("c");
  });
});

describe("element registry", () => {
  it("keeps the record of the children setElementText replaces", () => {
    const root = createElementNode("div");
    const child = createElementNode("div");
    const grandchild = createElementNode("div");
    insert(grandchild, child);
    insert(child, root);

    setElementText(root, "hello");

    expect(elementOf(child.__node_id)).toBe(child);
    expect(elementOf(grandchild.__node_id)).toBe(grandchild);
  });
});

describe("the tail of splitText", () => {
  it("builds the tail without a link read", () => {
    const parent = createElementNode("div");
    const text = createTextNode("abcd");
    insert(text, parent);
    fake.spies.linkedNode.mockClear();

    text.splitText(2);

    expect(fake.spies.linkedNode).not.toHaveBeenCalled();
  });

  it("keeps the handle of a tail that has a parent after a collection", async () => {
    const parent = createElementNode("div");
    const id = (() => {
      const text = createTextNode("abcd");
      insert(text, parent);
      return text.splitText(2).__node_id;
    })();

    await collectSentinel();

    expect(nodeOf(id)).toBeDefined();
  });

  it("holds a tail that has no parent", () => {
    const tail = createTextNode("abcd").splitText(2);

    expect(fake.spies.holdNode).toHaveBeenCalledWith(tail.__node_id);
  });
});

describe("the handles after the textContent setter", () => {
  it("lets the old children be collected", async () => {
    const el = createElementNode("div");
    const old = (() => {
      const child = createElementNode("span");
      insert(child, el);
      return new WeakRef(child);
    })();

    el.textContent = "new";
    await collectGarbage(() => old.deref() === undefined);

    expect(old.deref()).toBeUndefined();
  });

  it("builds no handle for the new text node", () => {
    const el = createElementNode("div");

    el.textContent = "new";

    expect(nodeOf(fake.nodes.get(el.__node_id)!.children[0]!)).toBeUndefined();
  });
});
