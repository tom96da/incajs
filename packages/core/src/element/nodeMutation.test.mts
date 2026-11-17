// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { installFakeHost } from "../../tests/fakeHost.mts";
import { collectGarbage, collectSentinel } from "../../tests/gc.mts";
import { HTML_NAMESPACE } from "../ffi/index.mts";
import { createElementNode, createIncaElement, createIncaFragment } from "./handle.mts";
import { isHeld, isRemoved, markFreed, nodeOf } from "./lifecycle.mts";
import { remove } from "./treeOps.mts";
import type { FakeHost } from "../../tests/fakeHost.mts";
import type { Node } from "./nodeTypes.mts";
import type { DocumentFragment, HTMLTemplateElement } from "./types.mts";

let host: FakeHost;

beforeEach(() => {
  vi.clearAllMocks();
  host = installFakeHost();
});

const div = () => createElementNode("div");
const fragment = (): DocumentFragment => createIncaFragment(host.create("#fragment"));
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

    expect(kids(parent).map((node) => node.__node_id)).toEqual(
      [...children, child].map((node) => node.__node_id),
    );
  });

  it("sets the parent of the node", () => {
    const { parent } = family(2);
    const child = div();

    parent.appendChild(child);

    expect(child.parentNode).toBe(parent);
  });

  it("moves a node from another parent", () => {
    const { children } = family(2);
    const { parent: to } = family(1);

    to.appendChild(children[0]!);

    expect(children[0]!.parentNode).toBe(to);
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

    expect(kids(parent).map((node) => node!.__node_id)).toEqual(
      [children[1], children[2], children[0]].map((node) => node!.__node_id),
    );
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

    expect(kids(parent).map((node) => node.__node_id)).toEqual(
      [...children, first, second].map((node) => node.__node_id),
    );
    expect(kids(group).map((node) => node.__node_id)).toEqual([]);
    expect(first.parentNode).toBe(parent);
    expect(second.parentNode).toBe(parent);
  });
});

describe("insertBefore", () => {
  it("puts the node before the reference", () => {
    const { parent, children } = family(2);
    const child = div();

    parent.insertBefore(child, children[1]!);

    expect(kids(parent).map((node) => node!.__node_id)).toEqual(
      [children[0], child, children[1]].map((node) => node!.__node_id),
    );
    expect(child.parentNode).toBe(parent);
  });

  it.each([null, undefined])("appends for the reference %s", (reference) => {
    const { parent, children } = family(2);
    const child = div();

    parent.insertBefore(child, reference);

    expect(kids(parent).map((node) => node.__node_id)).toEqual(
      [...children, child].map((node) => node.__node_id),
    );
  });

  it("sends null to the host for a missing reference", () => {
    const { parent } = family(1);
    const child = div();

    parent.insertBefore(child, undefined);

    expect(host.spies.insertBefore).toHaveBeenLastCalledWith(
      parent.__node_id,
      child.__node_id,
      null,
    );
  });

  it("counts the position after the node left its place", () => {
    const { parent, children } = family(3);

    parent.insertBefore(children[0]!, children[2]!);

    expect(kids(parent).map((node) => node!.__node_id)).toEqual(
      [children[1], children[0], children[2]].map((node) => node!.__node_id),
    );
    expect(idsOf(parent)).toHaveLength(3);
  });

  it("keeps the order when a node is its own reference", () => {
    const { parent, children } = family(3);

    parent.insertBefore(children[1]!, children[1]!);

    expect(kids(parent).map((node) => node.__node_id)).toEqual(
      children.map((node) => node.__node_id),
    );
  });

  it("moves the children of a fragment before the reference", () => {
    const { parent, children } = family(2);
    const group = fragment();
    const first = div();
    const second = div();
    group.appendChild(first);
    group.appendChild(second);

    parent.insertBefore(group, children[1]!);

    expect(kids(parent).map((node) => node!.__node_id)).toEqual(
      [children[0], first, second, children[1]].map((node) => node!.__node_id),
    );
    expect(kids(group).map((node) => node.__node_id)).toEqual([]);
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

    expect(kids(parent).map((node) => node!.__node_id)).toEqual(
      [children[0], child, children[2]].map((node) => node!.__node_id),
    );
    expect(old).toBe(children[1]);
    expect(idsOf(parent)).toHaveLength(3);
  });

  it("unlinks the replaced child", () => {
    const { parent, children } = family(2);

    parent.replaceChild(div(), children[0]!);

    expect(children[0]!.parentNode).toBeNull();
  });

  it("holds the replaced child", () => {
    const { parent, children } = family(1);

    parent.replaceChild(div(), children[0]!);

    expect(host.spies.holdNode).toHaveBeenCalledWith(children[0]!.__node_id);
    expect(isHeld(children[0]!)).toBe(true);
  });

  it("keeps the listeners of the replaced child", () => {
    const { parent, children } = family(1);

    parent.replaceChild(div(), children[0]!);

    expect(host.spies.removeNode).not.toHaveBeenCalled();
    expect(isRemoved(children[0]!)).toBe(false);
  });

  it("leaves the tree as it was when the node is the child", () => {
    const { parent, children } = family(3);

    parent.replaceChild(children[1]!, children[1]!);

    expect(kids(parent).map((node) => node.__node_id)).toEqual(
      children.map((node) => node.__node_id),
    );
    expect(children[1]!.parentNode).toBe(parent);
    expect(idsOf(parent)).toHaveLength(3);
    expect(host.spies.holdNode).not.toHaveBeenCalled();
  });

  it("takes a descendant of the child out of it", () => {
    const { parent, children } = family(2);
    const inner = div();
    children[0]!.appendChild(inner);

    parent.replaceChild(inner, children[0]!);

    expect(kids(parent).map((node) => node!.__node_id)).toEqual(
      [inner, children[1]].map((node) => node!.__node_id),
    );
    expect(kids(children[0]!).map((node) => node.__node_id)).toEqual([]);
    expect(idsOf(parent)).toHaveLength(2);
  });

  it("counts the position without a node before the child", () => {
    const { parent, children } = family(3);

    parent.replaceChild(children[0]!, children[1]!);

    expect(kids(parent).map((node) => node!.__node_id)).toEqual(
      [children[0], children[2]].map((node) => node!.__node_id),
    );
    expect(idsOf(parent)).toHaveLength(2);
  });

  it("puts a node after the child where the child was", () => {
    const { parent, children } = family(3);

    parent.replaceChild(children[2]!, children[1]!);

    expect(kids(parent).map((node) => node!.__node_id)).toEqual(
      [children[0], children[2]].map((node) => node!.__node_id),
    );
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

    expect(kids(parent).map((node) => node!.__node_id)).toEqual(
      [first, second, children[1]].map((node) => node!.__node_id),
    );
    expect(kids(group).map((node) => node.__node_id)).toEqual([]);
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

    expect(kids(parent).map((node) => node!.__node_id)).toEqual(
      [children[1]].map((node) => node!.__node_id),
    );
    expect(children[0]!.parentNode).toBeNull();
  });

  it("holds the child", () => {
    const { parent, children } = family(1);

    parent.removeChild(children[0]!);

    expect(host.spies.holdNode).toHaveBeenCalledWith(children[0]!.__node_id);
    expect(isHeld(children[0]!)).toBe(true);
  });

  it("keeps the listeners of the child", () => {
    const { parent, children } = family(1);

    parent.removeChild(children[0]!);

    expect(host.spies.removeNode).not.toHaveBeenCalled();
    expect(isRemoved(children[0]!)).toBe(false);
  });

  it("lets the child be appended again", () => {
    const { parent, children } = family(1);
    parent.removeChild(children[0]!);

    parent.appendChild(children[0]!);

    expect(kids(parent).map((node) => node.__node_id)).toEqual(
      children.map((node) => node.__node_id),
    );
    expect(idsOf(parent)).toHaveLength(1);
  });

  it("lets remove() take a node out of its fragment", () => {
    const group = fragment();
    const child = div();
    group.appendChild(child);

    remove(child);

    expect(kids(group).map((node) => node.__node_id)).toEqual([]);
    expect(host.spies.removeNode).toHaveBeenCalledWith(child.__node_id);
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

    expect(() => (parent.insertBefore as (node: Node) => void)(div())).toThrow(TypeError);
  });

  it("replaceChild throws a TypeError without a child argument", () => {
    const parent = div();

    expect(() => (parent.replaceChild as (node: Node) => void)(div())).toThrow(TypeError);
  });

  it.each([
    ["appendChild", (p: Node) => p.appendChild({} as Node)],
    ["insertBefore node", (p: Node) => p.insertBefore({} as Node, null)],
    ["insertBefore reference", (p: Node) => p.insertBefore(div(), {} as Node)],
    ["removeChild", (p: Node) => p.removeChild({} as Node)],
    ["replaceChild node", (p: Node) => p.replaceChild({} as Node, div())],
    ["replaceChild child", (p: Node) => p.replaceChild(div(), {} as Node)],
  ])("%s throws a TypeError for an argument that is not a node", (_, call) => {
    expect(() => call(div())).toThrow(TypeError);
  });

  it("throws NotFoundError from removeChild for a freed child", () => {
    const { parent, children } = family(1);
    markFreed([children[0]!.__node_id]);

    expect(() => parent.removeChild(children[0]!)).toThrow(
      expect.objectContaining({ name: "NotFoundError" }),
    );
  });

  it("throws NotFoundError from insertBefore for a freed reference", () => {
    const { parent, children } = family(1);
    markFreed([children[0]!.__node_id]);

    expect(() => parent.insertBefore(div(), children[0]!)).toThrow(
      expect.objectContaining({ name: "NotFoundError" }),
    );
  });

  it("throws NotFoundError from replaceChild for a freed child", () => {
    const { parent, children } = family(1);
    markFreed([children[0]!.__node_id]);

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

    expect(kids(first).map((node) => node.__node_id)).toEqual(
      children.map((node) => node.__node_id),
    );
    expect(children[0]!.parentNode).toBe(first);
    expect(kids(second)).toHaveLength(1);
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

    expect(kids(first).map((node) => node.__node_id)).toEqual(
      children.map((node) => node.__node_id),
    );
    expect(children[0]!.parentNode).toBe(first);
    expect(kids(group).map((node) => node.__node_id)).toEqual(
      [inside].map((node) => node.__node_id),
    );
    expect(kids(second).map((node) => node.__node_id)).toEqual(
      others.map((node) => node.__node_id),
    );
  });

  it.each([
    ["an element", (children: Node[]) => children[0]!],
    ["a fragment", (_children: Node[], group: Node) => group],
  ])("changes no list when insertBefore of %s gets a stray reference", (_name, pick) => {
    const { parent: first, children } = family(2);
    const { parent: second, children: others } = family(1);
    const group = fragment();
    const inside = div();
    group.appendChild(inside);

    expect(() => second.insertBefore(pick(children, group), children[1]!)).toThrow(
      expect.objectContaining({ name: "NotFoundError" }),
    );

    expect(kids(first).map((node) => node.__node_id)).toEqual(
      children.map((node) => node.__node_id),
    );
    expect(children[0]!.parentNode).toBe(first);
    expect(kids(group).map((node) => node.__node_id)).toEqual(
      [inside].map((node) => node.__node_id),
    );
    expect(inside.parentNode).toBe(group);
    expect(kids(second).map((node) => node.__node_id)).toEqual(
      others.map((node) => node.__node_id),
    );
  });
});

describe("a template", () => {
  const template = () => createElementNode("template") as HTMLTemplateElement;

  it("takes an appended node as its own child and not as content", () => {
    const owner = template();
    const child = div();

    owner.appendChild(child);

    expect(kids(owner).map((node) => node.__node_id)).toEqual([child.__node_id]);
    expect(kids(owner.content)).toEqual([]);
  });

  it("makes no content for the copy of an SVG template and keeps the ids in step", () => {
    const svgNamespace = "http://www.w3.org/2000/svg";
    const owner = createIncaElement(host.create("div"), [
      "template",
      svgNamespace,
      null,
      "template",
    ]);
    owner.appendChild(div());
    const [copyId, childId] = [host.create("div"), host.create("div")];
    host.natives.appendChild!(copyId, childId);
    host.spies.cloneNode.mockImplementationOnce(() => [
      [copyId, "template", svgNamespace, null, "template"],
      [childId, "div", HTML_NAMESPACE, null, "DIV"],
    ]);

    const copy = owner.cloneNode(true);

    expect(["content" in copy, kids(copy)[0]!.__node_id]).toEqual([false, childId]);
  });

  it("holds the copy of a cloned template once", () => {
    const owner = template();

    const copy = owner.cloneNode(false);

    const holds = host.spies.holdNode.mock.calls.filter(([id]) => id === copy.__node_id);
    expect(holds).toHaveLength(1);
  });

  it("gives a shallow clone an empty content with the id the host returned", () => {
    const owner = template();
    owner.content.appendChild(div());

    const copy = owner.cloneNode(false) as HTMLTemplateElement;

    expect([copy.content.__node_id, kids(copy.content)]).toEqual([copy.__node_id + 1, []]);
  });

  it("copies the content nodes and children of a deep clone from their own entries", () => {
    const owner = template();
    const inside = div();
    const own = div();
    owner.content.appendChild(inside);
    owner.appendChild(own);

    const copy = owner.cloneNode(true) as HTMLTemplateElement;

    const pairs = host.spies.cloneNode.mock.results[0]!.value as unknown[][];
    expect([
      kids(copy.content)[0]!.__node_id,
      kids(copy)[0]!.__node_id,
      kids(copy.content)[0] === inside,
    ]).toEqual([pairs[2]![0], pairs[3]![0], false]);
  });
});

describe("the handles after a mutation member", () => {
  it.each([
    ["appendChild", (parent: Node, child: Node) => parent.appendChild(child)],
    ["insertBefore", (parent: Node, child: Node) => parent.insertBefore(child, null)],
  ] as const)(
    "%s keeps the handle of the inserted node after a collection",
    async (_member, call) => {
      const parent = div();
      const id = (() => {
        const child = div();
        call(parent, child);
        return child.__node_id;
      })();

      await collectSentinel();

      expect(nodeOf(id)).toBeDefined();
    },
  );

  it("keeps the handles of the former children of an inserted fragment", async () => {
    const parent = div();
    const id = (() => {
      const group = fragment();
      const child = div();
      group.appendChild(child);
      parent.appendChild(group);
      return child.__node_id;
    })();

    await collectSentinel();

    expect(nodeOf(id)).toBeDefined();
  });

  it("keeps the handle of a node moved to another parent", async () => {
    const [from, to] = [div(), div()];
    const id = (() => {
      const child = div();
      from.appendChild(child);
      to.appendChild(child);
      return child.__node_id;
    })();

    await collectSentinel();

    expect(nodeOf(id)).toBeDefined();
  });

  it("lets the handle of a removed child be collected", async () => {
    const parent = div();
    const child = (() => {
      const made = div();
      parent.appendChild(made);
      parent.removeChild(made);
      return new WeakRef(made);
    })();

    await collectGarbage(() => child.deref() === undefined);

    expect(child.deref()).toBeUndefined();
  });

  it("lets the replaced child be collected", async () => {
    const parent = div();
    const old = (() => {
      const [first, second] = [div(), div()];
      parent.appendChild(second);
      parent.replaceChild(first, second);
      return new WeakRef(second);
    })();

    await collectGarbage(() => old.deref() === undefined);

    expect(old.deref()).toBeUndefined();
  });

  it("keeps the handle of the new node after a collection", async () => {
    const parent = div();
    const added = (() => {
      const [first, second] = [div(), div()];
      parent.appendChild(second);
      parent.replaceChild(first, second);
      return first.__node_id;
    })();

    await collectSentinel();

    expect(nodeOf(added)).toBeDefined();
  });
});
