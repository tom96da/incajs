// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { installFakeHost } from "../../tests/fakeHost.mts";
import { collectGarbage, collectSentinel } from "../../tests/gc.mts";
import { nativeStub } from "../../tests/setup.mts";
import { document } from "../document.mts";
import { createCommentNode, createElementNode, createTextNode, setElementText } from "./handle.mts";
import { elementOf, markFreed, nodeOf } from "./lifecycle.mts";
import { setText } from "./text.mts";
import { insert } from "./treeOps.mts";
import type { FakeHost } from "../../tests/fakeHost.mts";
import type { Node } from "./nodeTypes.mts";
import type { Text } from "./types.mts";

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
  wholeText: vi.fn<(nodeId: number) => string>(() => ""),
  substringData: vi.fn<(nodeId: number, offset: number, count: number) => string>(() => ""),
  replaceData: vi.fn<(nodeId: number, offset: number, count: number, data: string) => void>(),
  splitText: vi.fn<(nodeId: number, offset: number) => number>(),
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
  vi.stubGlobal("__inca_native__", nativeStub({ ...fake.natives, ...host } as never));
});

describe("data", () => {
  it("reads the data from the host", () => {
    const text = createTextNode("first");
    fake.nodes.get(text.__node_id)!.data = "second";

    expect(text.data).toBe("second");
  });

  it.each([
    [null, ""],
    [undefined, "undefined"],
    ["x", "x"],
  ] as const)("stores %s as %j", (value, stored) => {
    const text = createTextNode("first");

    text.data = value as string;

    expect(dataOf(text)).toBe(stored);
  });
});

describe("textContent of a text node", () => {
  it("reads the data", () => {
    expect(createTextNode("t").textContent).toBe("t");
  });

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

describe("textContent of a comment", () => {
  it("reads the data", () => {
    expect(createCommentNode("note").textContent).toBe("note");
  });
});

describe("length", () => {
  it("counts UTF-16 code units", () => {
    expect(createTextNode("a😀b").length).toBe(4);
  });
});

describe("substringData", () => {
  it("asks the host for the range", () => {
    const text = createTextNode("abc");
    host.substringData.mockReturnValueOnce("b");

    expect(text.substringData(1, 1)).toBe("b");
    expect(host.substringData).toHaveBeenCalledWith(text.__node_id, 1, 1);
  });

  it("reads the range of the last data of a freed node", () => {
    const text = createTextNode("abcd");
    markFreed([text.__node_id]);

    expect(text.substringData(1, 2)).toBe("bc");
  });
});

describe("the editing methods", () => {
  it.each([
    ["appendData", (text: Text) => text.appendData("X"), [3, 0, "X"]],
    ["insertData", (text: Text) => text.insertData(1, "X"), [1, 0, "X"]],
    ["deleteData", (text: Text) => text.deleteData(1, 2), [1, 2, ""]],
    ["replaceData", (text: Text) => text.replaceData(1, 2, "X"), [1, 2, "X"]],
  ] as const)("%s sends one replacement to the host", (_, call, args) => {
    const text = createTextNode("abc");

    call(text);

    expect(host.replaceData).toHaveBeenCalledWith(text.__node_id, ...args);
  });

  it("mirror the data the host holds in the text field", () => {
    const text = createTextNode("abc");
    host.replaceData.mockImplementationOnce((id) => {
      fake.nodes.get(id)!.data = "aXc";
    });

    text.replaceData(1, 1, "X");

    expect(text.text).toBe("aXc");
  });

  it("send nothing to the host for a freed node", () => {
    const text = createTextNode("abc");
    markFreed([text.__node_id]);

    text.appendData("X");

    expect(host.replaceData).not.toHaveBeenCalled();
  });

  it.each([
    ["substringData", []],
    ["appendData", []],
    ["insertData", [0]],
    ["deleteData", [0]],
    ["replaceData", [0, 0]],
    ["splitText", []],
  ] as const)("%s throws a TypeError without its arguments", (method, args) => {
    const call = createTextNode("abc")[method] as (...given: unknown[]) => unknown;

    expect(() => call(...args)).toThrow(TypeError);
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
    expect(tail.text).toBe("cd");
  });

  it("keeps the head in the text field", () => {
    const text = createTextNode("abcd");
    host.splitText.mockImplementationOnce((id, offset) => {
      fake.nodes.get(id)!.data = "ab";
      return fake.natives.splitText!(id, offset);
    });

    text.splitText(2);

    expect(text.text).toBe("ab");
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

describe("wholeText", () => {
  it("asks the host", () => {
    const text = createTextNode("a");
    host.wholeText.mockReturnValueOnce("abc");

    expect(text.wholeText).toBe("abc");
    expect(host.wholeText).toHaveBeenCalledWith(text.__node_id);
  });

  it("reads the last data of a freed node", () => {
    const text = createTextNode("a");
    markFreed([text.__node_id]);

    expect(text.wholeText).toBe("a");
  });
});

describe("textContent of an element", () => {
  it("asks the host", () => {
    const el = createElementNode("div");
    host.textContent.mockReturnValueOnce("abc");

    expect(el.textContent).toBe("abc");
    expect(host.textContent).toHaveBeenCalledWith(el.__node_id);
  });

  it("reads an empty text for a freed element", () => {
    const el = createElementNode("div");
    markFreed([el.__node_id]);

    expect(el.textContent).toBe("");
  });

  it("replaces the children with one text node", () => {
    const el = createElementNode("div");
    insert(createTextNode("old"), el);

    el.textContent = "new";

    expect(kids(el).map((child) => (child as Text).text)).toEqual(["new"]);
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
    expect(node).toMatchObject({ kind: "text", text: "hello" });
    expect("id" in node).toBe(false);
  });
});

describe("setText", () => {
  it("forwards the new value and updates the node", () => {
    const node = createTextNode("old") as Text;
    setText(node, "new");
    expect(host.setCharacterData).toHaveBeenCalledWith(node.__node_id, "new");
    expect(node.text).toBe("new");
  });
});

describe("setElementText", () => {
  it("replaces existing children with one text child", () => {
    const el = createElementNode("div");
    const oldChild = createElementNode("div");
    insert(oldChild, el);

    setElementText(el, "hello");

    expect(fake.spies.removeChild).toHaveBeenCalledWith(el.__node_id, oldChild.__node_id);
    expect(oldChild.parentNode).toBeNull();
    expect(kids(el)).toHaveLength(1);
    expect(kids(el)[0]).toMatchObject({ kind: "text", text: "hello" });
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
    host.setCharacterData.mockImplementationOnce(() => {
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
  it("reads the parent of the tail with one link read", () => {
    const parent = createElementNode("div");
    const text = createTextNode("abcd");
    insert(text, parent);
    fake.spies.linkedNode.mockClear();

    text.splitText(2);

    expect(fake.spies.linkedNode.mock.calls).toEqual([
      [host.splitText.mock.results[0]!.value, "parent", undefined],
    ]);
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

  it("keeps the handle of the new text node after a collection", async () => {
    const el = createElementNode("div");

    el.textContent = "new";
    await collectSentinel();

    expect(nodeOf(fake.nodes.get(el.__node_id)!.children[0]!)).toBeDefined();
  });
});
