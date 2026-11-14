// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import {
  createCommentNode,
  createElementNode,
  createIncaFragment,
  createTextNode,
} from "./handle.mts";
import { isHeld } from "./lifecycle.mts";
import type { ElementEntry, NodeEntry } from "../ffi/index.mts";
import type { IncaElement, IncaFragment, IncaNode } from "./types.mts";

/** The children of each node in the fake host. */
let kids = new Map<number, number[]>();
let parents = new Map<number, number>();
let tags = new Map<number, string>();
/** The content fragment of each template in the fake host. */
let contents = new Map<number, number>();
let nextId = 1;

const childrenOf = (id: number): number[] => kids.get(id)!;
const detach = (id: number): void => {
  const parent = parents.get(id);
  if (parent === undefined) return;
  childrenOf(parent).splice(childrenOf(parent).indexOf(id), 1);
  parents.delete(id);
};
const movedBy = (id: number): number[] =>
  tags.get(id) === "#fragment" ? [...childrenOf(id)] : [id];
/** Puts the nodes at the end of `parent` the way the host does and returns the position. */
const append = (parent: number, nodes: number[]): number => {
  for (const id of nodes) detach(id);
  const at = childrenOf(parent).length;
  childrenOf(parent).splice(at, 0, ...nodes);
  for (const id of nodes) parents.set(id, parent);
  return at;
};

const host = {
  createNode: vi.fn<(tag: string) => number>((tag) => {
    const id = nextId++;
    kids.set(id, []);
    tags.set(id, tag);
    if (tag === "template") {
      const content = nextId++;
      kids.set(content, []);
      tags.set(content, "#fragment");
      contents.set(id, content);
    }
    return id;
  }),
  templateContent: vi.fn<(nodeId: number) => number | null>((id) => contents.get(id) ?? null),
  setAttribute: vi.fn<() => void>(),
  setStyle: vi.fn<() => void>(),
  cloneNode: vi.fn<(nodeId: number, deep: boolean) => (NodeEntry | ElementEntry)[]>((id, deep) => {
    const copy = (original: number): [number, string][] => {
      const tag = tags.get(original)!;
      const made = host.createNode(tag);
      const pairs: [number, string][] = [[made, tag]];
      const content = contents.get(original);
      if (content !== undefined) {
        pairs.push([contents.get(made)!, "#fragment"]);
        if (deep) for (const child of childrenOf(content)) pairs.push(...copy(child));
      }
      if (deep) for (const child of childrenOf(original)) pairs.push(...copy(child));
      return pairs;
    };
    return copy(id);
  }),
  appendChild: vi.fn<(parentId: number, childId: number) => number>((parent, id) =>
    append(parent, movedBy(id)),
  ),
  holdNode: vi.fn<(nodeId: number) => void>(),
  releaseNode: vi.fn<(nodeId: number) => void>(),
  releaseListeners: vi.fn<(nodeId: number) => number[]>(() => []),
};

beforeEach(() => {
  vi.clearAllMocks();
  kids = new Map();
  parents = new Map();
  tags = new Map();
  contents = new Map();
  nextId = 1;
  globalThis.__inca_native__ = nativeStub(host as never);
});

const div = () => createElementNode("div");
const fragment = (): IncaFragment => createIncaFragment(host.createNode("#fragment"));
/** The ids the shadow lists of `node` hold, after checking that the host lists the same. */
const idsOf = (node: Exclude<IncaNode, { kind: "text" }>): number[] => {
  const ids = node.__children.map((child) => child.__node_id);
  expect(childrenOf(node.__node_id)).toEqual(ids);
  return ids;
};
/** A parent holding `count` fresh elements. */
const family = (count: number) => {
  const parent = div();
  const children = Array.from({ length: count }, div);
  for (const child of children) parent.appendChild(child);
  return { parent, children };
};

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
  const kinds = (nodes: readonly IncaNode[]) => nodes.map((node) => node.kind);

  it.each([
    ["undefined", undefined, false],
    ["true", true, true],
    ["a truthy value", 1, true],
  ])("asks the host for %s as the deep flag", (_name, deep, asked) => {
    const { parent } = family(0);

    parent.cloneNode(deep as boolean | undefined);

    expect(host.cloneNode).toHaveBeenCalledWith(parent.__node_id, asked);
  });

  it("builds an element with the tag the host returns", () => {
    const copy = createElementNode("span").cloneNode();

    expect(copy).toMatchObject({ kind: "element", tagName: "SPAN" });
  });

  it("builds an element with the name parts of a five-part entry", () => {
    const original = createElementNode("div");
    host.cloneNode.mockImplementationOnce(() => [
      [host.createNode("div"), "Y", "urn:x", "x", "X:Y"],
    ]);

    const copy = original.cloneNode() as IncaElement;

    expect([copy.tagName, copy.localName, copy.namespaceURI, copy.prefix]).toEqual([
      "X:Y",
      "Y",
      "urn:x",
      "x",
    ]);
  });

  it("builds a comment as a comment", () => {
    const copy = createCommentNode("c").cloneNode();

    expect(copy.kind).toBe("comment");
  });

  it("gives a comment the default tag name", () => {
    const copy = createCommentNode("c").cloneNode() as unknown as { tagName: string };

    expect(copy.tagName).toBe("DIV");
  });

  it("builds a text node with the data of the original", () => {
    const copy = createTextNode("x").cloneNode();

    expect(copy).toMatchObject({ kind: "text", text: "x" });
  });

  it("builds a fragment", () => {
    const copy = fragment().cloneNode();

    expect(copy.kind).toBe("fragment");
  });

  it("gives the copy new ids and no parent", () => {
    const { parent } = family(0);
    const copy = parent.cloneNode();

    expect([copy.__node_id === parent.__node_id, copy.parent]).toEqual([false, null]);
  });

  it("holds the copy", () => {
    expect(isHeld(div().cloneNode())).toBe(true);
  });

  it("copies no child in a shallow clone", () => {
    const { parent } = tree();

    expect(parent.cloneNode(false).__children).toEqual([]);
  });

  it("copies the descendants in a deep clone", () => {
    const { parent } = tree();
    const outer = div();
    outer.appendChild(parent);
    host.appendChild.mockClear();

    const copy = parent.cloneNode(true);

    expect(host.appendChild).not.toHaveBeenCalled();
    expect(idsOf(outer)).toEqual([parent.__node_id]);
    expect(kinds(copy.__children)).toEqual(["element", "text"]);
    expect(kinds((copy.__children[0] as typeof parent).__children)).toEqual(["comment"]);
  });

  it("gives each handle of a deep clone the id the host returned in pre-order", () => {
    const { parent } = tree();

    const copy = parent.cloneNode(true);
    const [inner, text] = copy.__children as [typeof parent, IncaNode];

    expect([inner.__node_id, inner.__children[0]!.__node_id, text.__node_id]).toEqual([
      copy.__node_id + 1,
      copy.__node_id + 2,
      copy.__node_id + 3,
    ]);
  });

  it("sets the parent of each copied child", () => {
    const { parent } = tree();

    const copy = parent.cloneNode(true);
    const inner = copy.__children[0] as typeof parent;

    expect([copy.__children.map((child) => child.parent), inner.__children[0]!.parent]).toEqual([
      [copy, copy],
      inner,
    ]);
  });

  it("copies the children of a fragment", () => {
    const group = fragment();
    group.appendChild(div());

    expect(kinds(group.cloneNode(true).__children)).toEqual(["element"]);
  });
});
