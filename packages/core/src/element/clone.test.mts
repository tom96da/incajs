// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { adoptFragment } from "../../tests/adopt.mts";
import { installFakeHost } from "../../tests/fakeHost.mts";
import { collectSentinel } from "../../tests/gc.mts";
import { createCommentNode, createElementNode, createTextNode } from "./handle.mts";
import { isHeld, nodeOf } from "./lifecycle.mts";
import type { FakeHost } from "../../tests/fakeHost.mts";
import type { HandleNode } from "./handleNode.mts";
import type { Node } from "./nodeTypes.mts";
import type { Element, DocumentFragment } from "./types.mts";

let host: FakeHost;

beforeEach(() => {
  vi.clearAllMocks();
  host = installFakeHost();
});

const div = () => createElementNode("div");
const fragment = (): DocumentFragment => adoptFragment(host.create("#fragment"));
/** The children of `node` as the host lists them. */
const kids = (node: Node): Node[] => Array.from(node.childNodes);
/** The ids of the children of `node`, after checking that the host lists the same. */
const idsOf = (node: Node): number[] => {
  const ids = kids(node).map((child) => child.__node_id);
  expect(host.nodes.get(node.__node_id)!.children).toEqual(ids);
  return ids;
};
/** A parent holding `count` fresh elements. */
const family = (count: number) => {
  const parent = div();
  const children = Array.from({ length: count }, div);
  for (const child of children) parent.appendChild(child);
  return { parent, children };
};

/** Replaces `nodeHandle` of the installed host by a spy that calls it. */
function countNodeHandle() {
  const real = globalThis.__inca_native__;
  const spy = vi.fn<(id: number) => ReturnType<typeof real.nodeHandle>>((id) =>
    real.nodeHandle(id),
  );
  vi.stubGlobal("__inca_native__", { ...real, nodeHandle: spy });
  return spy;
}

describe("cloneNode", () => {
  /** An element holding a text node and an element that holds a comment. */
  const tree = () => {
    const { parent, children } = family(1);
    const text = createTextNode("x");
    const comment = createCommentNode("c");
    parent.appendChild(text);
    children[0]!.appendChild(comment);
    return { parent, inner: children[0]!, text, comment };
  };
  const kinds = (nodes: readonly Node[]) => nodes.map((node) => (node as HandleNode).kind);

  it.each([
    ["undefined", undefined, false],
    ["true", true, true],
    ["a truthy value", 1, true],
  ])("asks the host for %s as the deep flag", (_name, deep, asked) => {
    const { parent } = family(0);

    parent.cloneNode(deep as boolean | undefined);

    expect(host.spies.cloneNode).toHaveBeenCalledWith(parent.__node_id, asked);
  });

  it("builds an element", () => {
    const copy = createElementNode("span").cloneNode();

    expect(copy).toMatchObject({ kind: "element", tagName: "SPAN" });
  });

  it("builds a comment as a comment", () => {
    const copy = createCommentNode("c").cloneNode();

    expect(copy.kind).toBe("comment");
  });

  it("builds a text node with the data of the original", () => {
    const copy = createTextNode("x").cloneNode();

    expect(copy).toMatchObject({ kind: "text", text: "x" });
  });

  it("reuses the content handle that the copy of a template made", () => {
    const copy = createElementNode("template").cloneNode(true) as unknown as Element & {
      content: Node;
    };

    expect(nodeOf(copy.content.__node_id)).toBe(copy.content);
  });

  it("builds a fragment", () => {
    const copy = fragment().cloneNode();

    expect(copy.kind).toBe("fragment");
  });

  it("gives the copy new ids and no parent", () => {
    const { parent } = family(0);
    const copy = parent.cloneNode();

    expect([copy.__node_id === parent.__node_id, copy.parentNode]).toEqual([false, null]);
  });

  it("holds the copy", () => {
    expect(isHeld(div().cloneNode())).toBe(true);
  });

  it("builds one handle for a deep clone", () => {
    const { parent } = tree();
    const nodeHandle = countNodeHandle();

    parent.cloneNode(true);

    expect(nodeHandle).toHaveBeenCalledTimes(1);
  });

  it("builds the handle of a descendant of the copy when it is read", () => {
    const { parent } = tree();
    const copy = parent.cloneNode(true);
    const nodeHandle = countNodeHandle();

    const inner = kids(copy)[0]!;

    expect(nodeHandle).toHaveBeenCalledWith(inner.__node_id);
    expect(inner.parentNode).toBe(copy);
  });

  it("copies no child in a shallow clone", () => {
    const { parent } = tree();

    expect(kids(parent.cloneNode(false))).toEqual([]);
  });

  it("copies the descendants in a deep clone", () => {
    const { parent } = tree();
    const outer = div();
    outer.appendChild(parent);
    host.spies.insertBefore.mockClear();

    const copy = parent.cloneNode(true);

    expect(host.spies.insertBefore).not.toHaveBeenCalled();
    expect(idsOf(outer)).toEqual([parent.__node_id]);
    expect(kinds(kids(copy))).toEqual(["element", "text"]);
    expect(kinds(kids(kids(copy)[0]!))).toEqual(["comment"]);
  });

  it("gives each handle of a deep clone the id the host returned in pre-order", () => {
    const { parent } = tree();

    const copy = parent.cloneNode(true);
    const [inner, text] = kids(copy) as [typeof parent, Node];

    expect([inner.__node_id, kids(inner)[0]!.__node_id, text.__node_id]).toEqual([
      copy.__node_id + 1,
      copy.__node_id + 2,
      copy.__node_id + 3,
    ]);
  });

  it("reads the copy as the parent of each copied child", () => {
    const { parent } = tree();

    const copy = parent.cloneNode(true);
    const inner = kids(copy)[0] as typeof parent;

    expect([kids(copy).map((child) => child.parentNode), kids(inner)[0]!.parentNode]).toEqual([
      [copy, copy],
      inner,
    ]);
  });

  it("copies the children of a fragment", () => {
    const group = fragment();
    group.appendChild(div());

    expect(kinds(kids(group.cloneNode(true)))).toEqual(["element"]);
  });
});

describe("the handles of a copy after a collection", () => {
  it("keeps the descendants of a deep copy", async () => {
    const original = div();
    original.appendChild(div());
    const id = (() => kids(original.cloneNode(true))[0]!.__node_id)();

    await collectSentinel();

    expect(nodeOf(id)).toBeDefined();
  });
});
