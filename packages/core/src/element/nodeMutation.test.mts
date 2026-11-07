// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { createCommentNode, createElementNode, createIncaFragment } from "./handle.mts";
import { isHeld, isRemoved, markDestroyed } from "./lifecycle.mts";
import { remove } from "./shadowTree.mts";
import { createTextNode } from "./text.mts";
import type { IncaFragment, IncaNode } from "./types.mts";

/** The children of each node in the fake host. */
let kids = new Map<number, number[]>();
let parents = new Map<number, number>();
let tags = new Map<number, string>();
let nextId = 1;

const notFound = () => new DOMException("not a child", "NotFoundError");
const childrenOf = (id: number): number[] => kids.get(id)!;
const nextOf = (id: number): number | null => {
  const siblings = childrenOf(parents.get(id)!);
  return siblings[siblings.indexOf(id) + 1] ?? null;
};
const detach = (id: number): void => {
  const parent = parents.get(id);
  if (parent === undefined) return;
  childrenOf(parent).splice(childrenOf(parent).indexOf(id), 1);
  parents.delete(id);
};
const movedBy = (id: number): number[] =>
  tags.get(id) === "#fragment" ? [...childrenOf(id)] : [id];
/** Puts the nodes before `reference` the way the host does and returns the position. */
const place = (parent: number, nodes: number[], reference: number | null): number => {
  for (const id of nodes) detach(id);
  const at = reference === null ? childrenOf(parent).length : childrenOf(parent).indexOf(reference);
  childrenOf(parent).splice(at, 0, ...nodes);
  for (const id of nodes) parents.set(id, parent);
  return at;
};

const host = {
  createNode: vi.fn<(tag: string) => number>((tag) => {
    const id = nextId++;
    kids.set(id, []);
    tags.set(id, tag);
    return id;
  }),
  setAttribute: vi.fn<() => void>(),
  setStyle: vi.fn<() => void>(),
  cloneNode: vi.fn<(nodeId: number, deep: boolean) => [number, string][]>((id, deep) => {
    const copy = (original: number): [number, string][] => {
      const tag = tags.get(original)!;
      const pairs: [number, string][] = [[host.createNode(tag), tag]];
      if (deep) for (const child of childrenOf(original)) pairs.push(...copy(child));
      return pairs;
    };
    return copy(id);
  }),
  appendChild: vi.fn<(parentId: number, childId: number) => number>((parent, id) =>
    place(parent, movedBy(id), null),
  ),
  insertBefore: vi.fn<(parentId: number, childId: number, refId: number | null) => number>(
    (parent, id, reference) => {
      if (reference !== null && !childrenOf(parent).includes(reference)) throw notFound();
      return place(parent, movedBy(id), reference === id ? nextOf(id) : reference);
    },
  ),
  removeChild: vi.fn<(parentId: number, childId: number) => void>((parent, id) => {
    if (parents.get(id) !== parent) throw notFound();
    detach(id);
  }),
  replaceChild: vi.fn<(parentId: number, nodeId: number, childId: number) => number>(
    (parent, id, child) => {
      if (parents.get(child) !== parent) throw notFound();
      let reference = nextOf(child);
      if (reference === id) reference = nextOf(id);
      detach(child);
      return place(parent, movedBy(id), reference);
    },
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
  nextId = 1;
  globalThis.__inca_native__ = host as never;
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

describe("appendChild", () => {
  it("returns the node", () => {
    const { parent } = family(0);
    const child = div();

    expect(parent.appendChild(child)).toBe(child);
  });

  it("puts the node last", () => {
    const { parent, children } = family(2);
    const child = div();

    parent.appendChild(child);

    expect(parent.__children).toEqual([...children, child]);
  });

  it("sets the parent of the node", () => {
    const { parent } = family(2);
    const child = div();

    parent.appendChild(child);

    expect(child.parent).toBe(parent);
  });

  it("moves a node from another parent", () => {
    const { children } = family(2);
    const { parent: to } = family(1);

    to.appendChild(children[0]!);

    expect(children[0]!.parent).toBe(to);
  });

  it("leaves the old parent without the moved node", () => {
    const { parent: from, children } = family(2);
    const { parent: to } = family(1);

    to.appendChild(children[0]!);

    expect(idsOf(from)).toEqual([children[1]!.__node_id]);
  });

  it("moves its first child to the end", () => {
    const { parent, children } = family(3);

    parent.appendChild(children[0]!);

    expect(parent.__children).toEqual([children[1], children[2], children[0]]);
    expect(idsOf(parent)).toHaveLength(3);
  });

  it("moves the children of a fragment and empties it", () => {
    const { parent, children } = family(1);
    const group = fragment();
    const first = div();
    const second = div();
    group.appendChild(first);
    group.appendChild(second);

    parent.appendChild(group);

    expect(parent.__children).toEqual([...children, first, second]);
    expect(group.__children).toEqual([]);
    expect(first.parent).toBe(parent);
    expect(second.parent).toBe(parent);
  });
});

describe("insertBefore", () => {
  it("puts the node before the reference", () => {
    const { parent, children } = family(2);
    const child = div();

    parent.insertBefore(child, children[1]!);

    expect(parent.__children).toEqual([children[0], child, children[1]]);
    expect(child.parent).toBe(parent);
  });

  it.each([null, undefined])("appends for the reference %s", (reference) => {
    const { parent, children } = family(2);
    const child = div();

    parent.insertBefore(child, reference);

    expect(parent.__children).toEqual([...children, child]);
  });

  it("sends null to the host for a missing reference", () => {
    const { parent } = family(1);
    const child = div();

    parent.insertBefore(child, undefined);

    expect(host.insertBefore).toHaveBeenLastCalledWith(parent.__node_id, child.__node_id, null);
  });

  it("counts the position after the node left its place", () => {
    const { parent, children } = family(3);

    parent.insertBefore(children[0]!, children[2]!);

    expect(parent.__children).toEqual([children[1], children[0], children[2]]);
    expect(idsOf(parent)).toHaveLength(3);
  });

  it("keeps the order when a node is its own reference", () => {
    const { parent, children } = family(3);

    parent.insertBefore(children[1]!, children[1]!);

    expect(parent.__children).toEqual(children);
  });

  it("moves the children of a fragment before the reference", () => {
    const { parent, children } = family(2);
    const group = fragment();
    const first = div();
    const second = div();
    group.appendChild(first);
    group.appendChild(second);

    parent.insertBefore(group, children[1]!);

    expect(parent.__children).toEqual([children[0], first, second, children[1]]);
    expect(group.__children).toEqual([]);
    expect(idsOf(parent)).toHaveLength(4);
  });

  it("returns the node", () => {
    const { parent } = family(0);
    const child = div();

    expect(parent.insertBefore(child, null)).toBe(child);
  });
});

describe("replaceChild", () => {
  it("puts the node where the child was and returns the child", () => {
    const { parent, children } = family(3);
    const child = div();

    const old = parent.replaceChild(child, children[1]!);

    expect(parent.__children).toEqual([children[0], child, children[2]]);
    expect(old).toBe(children[1]);
    expect(idsOf(parent)).toHaveLength(3);
  });

  it("unlinks the replaced child", () => {
    const { parent, children } = family(2);

    parent.replaceChild(div(), children[0]!);

    expect(children[0]!.parent).toBeNull();
  });

  it("holds the replaced child", () => {
    const { parent, children } = family(1);

    parent.replaceChild(div(), children[0]!);

    expect(host.holdNode).toHaveBeenCalledWith(children[0]!.__node_id);
    expect(isHeld(children[0]!)).toBe(true);
  });

  it("keeps the listeners of the replaced child", () => {
    const { parent, children } = family(1);

    parent.replaceChild(div(), children[0]!);

    expect(host.releaseListeners).not.toHaveBeenCalled();
    expect(isRemoved(children[0]!)).toBe(false);
  });

  it("leaves the tree as it was when the node is the child", () => {
    const { parent, children } = family(3);

    parent.replaceChild(children[1]!, children[1]!);

    expect(parent.__children).toEqual(children);
    expect(children[1]!.parent).toBe(parent);
    expect(idsOf(parent)).toHaveLength(3);
    expect(host.holdNode).not.toHaveBeenCalled();
  });

  it("takes a descendant of the child out of it", () => {
    const { parent, children } = family(2);
    const inner = div();
    children[0]!.appendChild(inner);

    parent.replaceChild(inner, children[0]!);

    expect(parent.__children).toEqual([inner, children[1]]);
    expect(children[0]!.__children).toEqual([]);
    expect(idsOf(parent)).toHaveLength(2);
  });

  it("counts the position without a node before the child", () => {
    const { parent, children } = family(3);

    parent.replaceChild(children[0]!, children[1]!);

    expect(parent.__children).toEqual([children[0], children[2]]);
    expect(idsOf(parent)).toHaveLength(2);
  });

  it("puts a node after the child where the child was", () => {
    const { parent, children } = family(3);

    parent.replaceChild(children[2]!, children[1]!);

    expect(parent.__children).toEqual([children[0], children[2]]);
    expect(idsOf(parent)).toHaveLength(2);
  });

  it("moves the children of a fragment into the place of the child", () => {
    const { parent, children } = family(2);
    const group = fragment();
    const first = div();
    const second = div();
    group.appendChild(first);
    group.appendChild(second);

    parent.replaceChild(group, children[0]!);

    expect(parent.__children).toEqual([first, second, children[1]]);
    expect(group.__children).toEqual([]);
    expect(idsOf(parent)).toHaveLength(3);
  });
});

describe("removeChild", () => {
  it("returns the child", () => {
    const { parent, children } = family(2);

    expect(parent.removeChild(children[0]!)).toBe(children[0]);
  });

  it("unlinks the child", () => {
    const { parent, children } = family(2);

    parent.removeChild(children[0]!);

    expect(parent.__children).toEqual([children[1]]);
    expect(children[0]!.parent).toBeNull();
  });

  it("holds the child", () => {
    const { parent, children } = family(1);

    parent.removeChild(children[0]!);

    expect(host.holdNode).toHaveBeenCalledWith(children[0]!.__node_id);
    expect(isHeld(children[0]!)).toBe(true);
  });

  it("keeps the listeners of the child", () => {
    const { parent, children } = family(1);

    parent.removeChild(children[0]!);

    expect(host.releaseListeners).not.toHaveBeenCalled();
    expect(isRemoved(children[0]!)).toBe(false);
  });

  it("lets the child be appended again", () => {
    const { parent, children } = family(1);
    parent.removeChild(children[0]!);

    parent.appendChild(children[0]!);

    expect(parent.__children).toEqual(children);
    expect(idsOf(parent)).toHaveLength(1);
  });

  it("lets remove() take a node out of its fragment", () => {
    const group = fragment();
    const child = div();
    group.appendChild(child);

    remove(child);

    expect(group.__children).toEqual([]);
    expect(host.removeChild).toHaveBeenCalledWith(group.__node_id, child.__node_id);
  });
});

describe("arguments", () => {
  it.each(["appendChild", "insertBefore", "removeChild", "replaceChild"] as const)(
    "%s throws a TypeError without arguments",
    (method) => {
      const parent = div();

      expect(() => (parent[method] as () => void)()).toThrow(TypeError);
    },
  );

  it("insertBefore throws a TypeError without a reference argument", () => {
    const parent = div();

    expect(() => (parent.insertBefore as (node: IncaNode) => void)(div())).toThrow(TypeError);
  });

  it("replaceChild throws a TypeError without a child argument", () => {
    const parent = div();

    expect(() => (parent.replaceChild as (node: IncaNode) => void)(div())).toThrow(TypeError);
  });

  it.each([
    ["appendChild", (p: IncaNode) => p.appendChild({} as IncaNode)],
    ["insertBefore node", (p: IncaNode) => p.insertBefore({} as IncaNode, null)],
    ["insertBefore reference", (p: IncaNode) => p.insertBefore(div(), {} as IncaNode)],
    ["removeChild", (p: IncaNode) => p.removeChild({} as IncaNode)],
    ["replaceChild node", (p: IncaNode) => p.replaceChild({} as IncaNode, div())],
    ["replaceChild child", (p: IncaNode) => p.replaceChild(div(), {} as IncaNode)],
  ])("%s throws a TypeError for an argument that is not a node", (_, call) => {
    expect(() => call(div())).toThrow(TypeError);
  });

  it("throws NotFoundError from removeChild for a freed child", () => {
    const { parent, children } = family(1);
    markDestroyed(children[0]!);

    expect(() => parent.removeChild(children[0]!)).toThrow(
      expect.objectContaining({ name: "NotFoundError" }),
    );
  });

  it("throws NotFoundError from insertBefore for a freed reference", () => {
    const { parent, children } = family(1);
    markDestroyed(children[0]!);

    expect(() => parent.insertBefore(div(), children[0]!)).toThrow(
      expect.objectContaining({ name: "NotFoundError" }),
    );
  });

  it("throws NotFoundError from replaceChild for a freed child", () => {
    const { parent, children } = family(1);
    markDestroyed(children[0]!);

    expect(() => parent.replaceChild(div(), children[0]!)).toThrow(
      expect.objectContaining({ name: "NotFoundError" }),
    );
  });

  it("throws NotFoundError from removeChild for a child of another parent", () => {
    const { children } = family(1);
    const { parent: second } = family(1);

    expect(() => second.removeChild(children[0]!)).toThrow(
      expect.objectContaining({ name: "NotFoundError" }),
    );
  });

  it("changes no list when removeChild throws", () => {
    const { parent: first, children } = family(1);
    const { parent: second } = family(1);

    expect(() => second.removeChild(children[0]!)).toThrow(
      expect.objectContaining({ name: "NotFoundError" }),
    );

    expect(first.__children).toEqual(children);
    expect(children[0]!.parent).toBe(first);
    expect(second.__children).toHaveLength(1);
  });

  it("changes no list when replaceChild gets a child of another parent", () => {
    const { parent: first, children } = family(2);
    const { parent: second, children: others } = family(1);
    const group = fragment();
    const inside = div();
    group.appendChild(inside);

    expect(() => second.replaceChild(group, children[0]!)).toThrow(
      expect.objectContaining({ name: "NotFoundError" }),
    );

    expect(first.__children).toEqual(children);
    expect(children[0]!.parent).toBe(first);
    expect(group.__children).toEqual([inside]);
    expect(second.__children).toEqual(others);
  });

  it.each([
    ["an element", (children: IncaNode[]) => children[0]!],
    ["a fragment", (_children: IncaNode[], group: IncaNode) => group],
  ])("changes no list when insertBefore of %s gets a stray reference", (_name, pick) => {
    const { parent: first, children } = family(2);
    const { parent: second, children: others } = family(1);
    const group = fragment();
    const inside = div();
    group.appendChild(inside);

    expect(() => second.insertBefore(pick(children, group), children[1]!)).toThrow(
      expect.objectContaining({ name: "NotFoundError" }),
    );

    expect(first.__children).toEqual(children);
    expect(children[0]!.parent).toBe(first);
    expect(group.__children).toEqual([inside]);
    expect(inside.parent).toBe(group);
    expect(second.__children).toEqual(others);
  });
});

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
