// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { createCommentNode, createElementNode } from "./handle.mts";
import { markDestroyed } from "./lifecycle.mts";
import { createTextNode, insert } from "./shadowTree.mts";
import type { IncaText } from "./types.mts";

/** The data of each text node or comment stand-in. */
let data: Record<number, string> = {};
let nextId = 100;

const host = {
  rootNodeId: vi.fn<() => number>(() => 0),
  createNode: vi.fn<(tag: string) => number>(() => nextId++),
  setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(
    (nodeId, _key, value) => {
      data[nodeId] = String(value);
    },
  ),
  getAttribute: vi.fn<(nodeId: number, key: string) => string | null>(
    (nodeId) => data[nodeId] ?? null,
  ),
  setStyle: vi.fn<() => void>(),
  appendChild: vi.fn<() => void>(),
  insertBefore: vi.fn<() => void>(),
  removeChild: vi.fn<() => void>(),
  releaseListeners: vi.fn<(nodeId: number) => number[]>(() => []),
  holdNode: vi.fn<(nodeId: number) => void>(),
  releaseNode: vi.fn<(nodeId: number) => void>(),
  textContent: vi.fn<(nodeId: number) => string>(() => ""),
  wholeText: vi.fn<(nodeId: number) => string>(() => ""),
  substringData: vi.fn<(nodeId: number, offset: number, count: number) => string>(() => ""),
  replaceData: vi.fn<(nodeId: number, offset: number, count: number, data: string) => void>(),
  splitText: vi.fn<(nodeId: number, offset: number) => number>(() => 900),
};

beforeEach(() => {
  vi.clearAllMocks();
  data = {};
  nextId = 100;
  globalThis.__inca_native__ = host as never;
});

describe("data", () => {
  it("reads the data from the host", () => {
    const text = createTextNode("first");
    data[text.__node_id] = "second";

    expect(text.data).toBe("second");
  });

  it.each([
    [null, ""],
    [undefined, "undefined"],
    ["x", "x"],
  ] as const)("stores %s as %j", (value, stored) => {
    const text = createTextNode("first");

    text.data = value as string;

    expect(data[text.__node_id]).toBe(stored);
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

    expect(data[text.__node_id]).toBe(stored);
  });
});

describe("textContent of a comment stand-in", () => {
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
    markDestroyed(text);

    expect(text.substringData(1, 2)).toBe("bc");
  });
});

describe("the editing methods", () => {
  it.each([
    ["appendData", (text: IncaText) => text.appendData("X"), [3, 0, "X"]],
    ["insertData", (text: IncaText) => text.insertData(1, "X"), [1, 0, "X"]],
    ["deleteData", (text: IncaText) => text.deleteData(1, 2), [1, 2, ""]],
    ["replaceData", (text: IncaText) => text.replaceData(1, 2, "X"), [1, 2, "X"]],
  ] as const)("%s sends one replacement to the host", (_, call, args) => {
    const text = createTextNode("abc");

    call(text);

    expect(host.replaceData).toHaveBeenCalledWith(text.__node_id, ...args);
  });

  it("mirror the data the host holds in the text field", () => {
    const text = createTextNode("abc");
    host.replaceData.mockImplementationOnce((id) => {
      data[id] = "aXc";
    });

    text.replaceData(1, 1, "X");

    expect(text.text).toBe("aXc");
  });

  it("send nothing to the host for a freed node", () => {
    const text = createTextNode("abc");
    markDestroyed(text);

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
    host.splitText.mockReturnValueOnce(900);
    data[900] = "cd";

    const tail = text.splitText(2);

    expect(tail.__node_id).toBe(900);
    expect(tail.text).toBe("cd");
  });

  it("keeps the head in the text field", () => {
    const text = createTextNode("abcd");
    host.splitText.mockImplementationOnce((id) => {
      data[id] = "ab";
      return 900;
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

    expect(parent.__children).toEqual([text, tail, last]);
    expect(tail.parent).toBe(parent);
  });

  it("leaves the tail with no parent for a node with no parent", () => {
    expect(createTextNode("abcd").splitText(2).parent).toBeNull();
  });

  it("throws a TypeError for a freed node", () => {
    const text = createTextNode("abcd");
    markDestroyed(text);

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
    markDestroyed(text);

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
    markDestroyed(el);

    expect(el.textContent).toBe("");
  });

  it("replaces the children with one text node", () => {
    const el = createElementNode("div");
    insert(createTextNode("old"), el);

    el.textContent = "new";

    expect(el.__children.map((child) => (child as IncaText).text)).toEqual(["new"]);
  });

  it.each([null, undefined, ""] as const)("removes the children for %j", (value) => {
    const el = createElementNode("div");
    insert(createTextNode("old"), el);

    el.textContent = value as string;

    expect(el.__children).toEqual([]);
  });

  it("releases the listeners of a removed child", () => {
    const el = createElementNode("div");
    const child = createElementNode("span");
    insert(child, el);

    el.textContent = "new";

    expect(host.releaseListeners).toHaveBeenCalledWith(child.__node_id);
  });

  it("ignores an assignment to a freed element", () => {
    const el = createElementNode("div");
    insert(createTextNode("old"), el);
    markDestroyed(el);

    el.textContent = "new";

    expect(el.__children).toHaveLength(1);
  });
});
