// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { installFakeHost } from "../../tests/fakeHost.mts";
import { collectGarbage, collectSentinel } from "../../tests/gc.mts";
import { keep } from "../../tests/keep.mts";
import { declareNode } from "../../tests/setup.mts";
import { createElementNode } from "./handle.mts";
import { markFreed, nodeOf } from "./lifecycle.mts";
import { insert } from "./treeOps.mts";
import type { FakeHost } from "../../tests/fakeHost.mts";
import type { Node } from "./nodeTypes.mts";
import type { Element, HTMLTemplateElement, Text } from "./types.mts";

let fake: FakeHost;

/** The children of `node` as the host lists them. */
const kids = (node: Node): Node[] => Array.from(node.childNodes);

/** Puts the parentless nodes of `ids` into `container` at `at`, as a parse does. */
function parsed(container: number, at: number, ids: number[]): void {
  const roots = ids.filter((id) => fake.nodes.get(id)!.parent === null);
  const before = fake.nodes.get(container)!.children[at] ?? null;
  for (const id of roots) fake.natives.insertBefore!(container, id, before);
}

/** Takes the children of a container out of it, as a parse that replaces them does. */
function clear(container: number): number[][] {
  const children = fake.nodes.get(container)!.children.slice();
  return children.flatMap((child) => fake.natives.removeChild!(container, child));
}

/** The node whose children a markup call replaces: the content of a template, else the node. */
const containerOf = (id: number): number => fake.nodes.get(id)!.content ?? id;

const host = {
  setInnerHtml: vi.fn<(nodeId: number, html: string) => number[][]>(),
  setOuterHtml: vi.fn<(nodeId: number, html: string) => number[][]>(),
  insertAdjacentHtml: vi.fn<(nodeId: number, position: string, html: string) => void>(),
};

beforeEach(() => {
  vi.clearAllMocks();
  fake = installFakeHost(host as never);
  host.setInnerHtml.mockImplementation((id) => clear(containerOf(id)));
  host.setOuterHtml.mockImplementation((id) =>
    fake.natives.removeChild!(fake.nodes.get(id)!.parent!, id),
  );
  host.insertAdjacentHtml.mockImplementation(() => undefined);
});

/** Makes the next `innerHTML` assignment parse into `ids`. */
const willParseInner = (ids: number[]): void =>
  void host.setInnerHtml.mockImplementationOnce((id) => {
    const removed = clear(containerOf(id));
    parsed(containerOf(id), 0, ids);
    return removed;
  });

const withChild = (): [Element, Element] => {
  const parent = createElementNode("div");
  const child = createElementNode("span");
  insert(child, parent);
  return [parent, child];
};
const SVG_NAMESPACE = "http://www.w3.org/2000/svg";
/** A parsed node the host made, below `parentId` when it names a node of the host. */
const entry = (tag: string, parentId = 0, namespace?: string): number => {
  const id = fake.create(tag);
  if (namespace !== undefined)
    declareNode(id, { kind: "element", name: [tag, namespace, null, tag] });
  if (parentId !== 0) fake.natives.appendChild!(parentId, id);
  return id;
};

describe("conversion of an assigned markup", () => {
  it.each([
    [null, ""],
    [undefined, "undefined"],
    [0, "0"],
  ] as const)("innerHTML takes %j as %j", (value, text) => {
    const el = createElementNode("div");

    el.innerHTML = value as never;

    expect(host.setInnerHtml).toHaveBeenCalledWith(el.__node_id, text);
  });

  it.each(["innerHTML", "outerHTML"] as const)("%s throws a TypeError for a Symbol", (member) => {
    const el = createElementNode("div");

    expect(() => {
      el[member] = Symbol("s") as never;
    }).toThrow(TypeError);
  });

  it("outerHTML takes null as an empty text", () => {
    const [, child] = withChild();

    child.outerHTML = null as never;

    expect(host.setOuterHtml).toHaveBeenCalledWith(child.__node_id, "");
  });

  it("insertAdjacentHTML takes null as the text null", () => {
    const el = createElementNode("div");

    el.insertAdjacentHTML("beforeend", null as never);

    expect(host.insertAdjacentHtml).toHaveBeenCalledWith(el.__node_id, "beforeend", "null");
  });
});

describe("insertAdjacentHTML", () => {
  it.each([[[]], [["beforeend"]]] as const)("needs two arguments, given %j", (args) => {
    const el = createElementNode("div");

    expect(() => (el.insertAdjacentHTML as (...a: unknown[]) => void)(...args)).toThrow(TypeError);
  });

  it.each([
    ["beforebegin", "the parent", 0],
    ["AfterEnd", "the parent", 0],
    ["afterbegin", "the element", 1],
    ["BEFOREEND", "the element", 1],
  ] as const)("enters the nodes of %s into %s", (position, _into, target) => {
    const [parent, child] = withChild();
    const made = entry("b");
    host.insertAdjacentHtml.mockImplementationOnce(() =>
      parsed([parent, child][target]!.__node_id, 0, [made]),
    );

    child.insertAdjacentHTML(position, "<b>");

    const container = [parent, child][target]!;
    expect(kids(container).map((node) => node.__node_id)).toContain(made);
  });
});

describe("the tree after a replacement", () => {
  it("enters the roots of innerHTML in order", () => {
    const [parent] = withChild();
    const first = entry("b");
    const second = entry("i");
    willParseInner([first, second]);

    parent.innerHTML = "<b></b><i></i>";

    expect(kids(parent).map((node) => node.__node_id)).toEqual([first, second]);
  });

  it("takes the old children out of innerHTML", () => {
    const [parent, old] = withChild();

    parent.innerHTML = "";

    expect([kids(parent).length, old.parentNode]).toEqual([0, null]);
  });

  it("keeps the listeners of an old child of innerHTML", () => {
    const [parent] = withChild();

    parent.innerHTML = "";

    expect(fake.spies.removeNode).not.toHaveBeenCalled();
  });

  it("holds an old child of innerHTML in the host", () => {
    const [parent, old] = withChild();

    parent.innerHTML = "";

    expect(fake.spies.holdNode).toHaveBeenCalledWith(old.__node_id);
  });

  it("replaces the element of outerHTML by its nodes", () => {
    const parent = createElementNode("div");
    const before = createElementNode("a");
    const target = createElementNode("b");
    insert(before, parent);
    insert(target, parent);
    const made = entry("i");
    host.setOuterHtml.mockImplementationOnce((id) => {
      const removed = fake.natives.removeChild!(parent.__node_id, id);
      parsed(parent.__node_id, 1, [made]);
      return removed;
    });

    target.outerHTML = "<i></i>";

    expect(kids(parent).map((node) => node.__node_id)).toEqual([before.__node_id, made]);
  });

  it("leaves the old element of outerHTML parentless", () => {
    const [, child] = withChild();

    child.outerHTML = "";

    expect(child.parentNode).toBeNull();
  });

  it("keeps the listeners of the old element of outerHTML", () => {
    const [, child] = withChild();

    child.outerHTML = "";

    expect(fake.spies.removeNode).not.toHaveBeenCalled();
  });

  it("holds the old element of outerHTML in the host", () => {
    const [, child] = withChild();

    child.outerHTML = "";

    expect(fake.spies.holdNode).toHaveBeenCalledWith(child.__node_id);
  });

  it("changes nothing for an outerHTML the host declines", () => {
    const [parent, child] = withChild();
    host.setOuterHtml.mockReturnValueOnce([]);

    child.outerHTML = "<i></i>";

    expect(kids(parent).map((node) => node.__node_id)).toEqual(
      [child].map((node) => node.__node_id),
    );
  });
});

describe("the handles of the parsed nodes", () => {
  it("reads the nested nodes through the host links", () => {
    const el = createElementNode("div");
    const outer = entry("p");
    const inner = entry("b", outer);
    const text = entry("#text", inner);
    fake.natives.setCharacterData!(text, "t");
    willParseInner([outer, inner, text]);

    el.innerHTML = "<p><b>t</b></p>";

    const p = kids(el)[0] as Element;
    const b = kids(p)[0] as Element;
    expect([kids(el).length, b.tagName, (kids(b)[0] as Text).textContent]).toEqual([1, "B", "t"]);
  });

  it("reaches the content of a parsed template", () => {
    const el = createElementNode("div");
    const template = entry("template");
    const child = entry("p", fake.nodes.get(template)!.content!);
    willParseInner([template]);

    el.innerHTML = "<template><p></p></template>";

    const made = kids(el)[0] as HTMLTemplateElement;
    expect([kids(made).length, kids(made.content)[0]?.__node_id]).toEqual([0, child]);
  });

  it("makes a parsed foreign input without value", () => {
    const el = createElementNode("div");
    willParseInner([entry("input", 0, SVG_NAMESPACE)]);

    el.innerHTML = "<svg><input></svg>";

    expect("value" in kids(el)[0]!).toBe(false);
  });
});

describe("a freed element", () => {
  it.each([
    ["innerHTML", (el: Element): void => void (el.innerHTML = "<b></b>"), () => host.setInnerHtml],
    ["outerHTML", (el: Element): void => void (el.outerHTML = "<b></b>"), () => host.setOuterHtml],
    [
      "insertAdjacentHTML",
      (el: Element) => el.insertAdjacentHTML("beforeend", "<b></b>"),
      () => host.insertAdjacentHtml,
    ],
  ] as const)("ignores the assignment of %s", (_member, assign, call) => {
    const el = createElementNode("div");
    markFreed([el.__node_id]);

    assign(el);

    expect(call()).not.toHaveBeenCalled();
  });
});

describe("the handles after a markup member", () => {
  /** Makes the next `outerHTML` assignment replace the node by `entries` at `at` of `parent`. */
  const willParseOuter = (parent: Element, ids: number[]): void =>
    void host.setOuterHtml.mockImplementationOnce((id) => {
      const removed = fake.natives.removeChild!(parent.__node_id, id);
      parsed(parent.__node_id, 0, ids);
      return removed;
    });

  it("lets the old children of innerHTML be collected", async () => {
    const parent = createElementNode("div");
    const old = (() => {
      const child = createElementNode("span");
      insert(child, parent);
      return new WeakRef(child);
    })();

    parent.innerHTML = "";
    await collectGarbage(() => old.deref() === undefined);

    expect(old.deref()).toBeUndefined();
  });

  it("keeps a read parsed handle of innerHTML after a collection", async () => {
    const el = createElementNode("div");
    const made = entry("b");
    willParseInner([made]);

    el.innerHTML = "<b></b>";
    kids(el);
    await collectSentinel();

    expect(nodeOf(made)).toBeDefined();
  });

  it("lets the element of outerHTML be collected", async () => {
    const parent = createElementNode("div");
    const old = (() => {
      const child = createElementNode("span");
      insert(child, parent);
      child.outerHTML = "";
      return new WeakRef(child);
    })();

    await collectGarbage(() => old.deref() === undefined);

    expect(old.deref()).toBeUndefined();
  });

  it("keeps a read parsed handle of outerHTML after a collection", async () => {
    const parent = createElementNode("div");
    const made = entry("i");
    (() => {
      const child = createElementNode("span");
      insert(child, parent);
      willParseOuter(parent, [made]);
      child.outerHTML = "<i></i>";
    })();
    kids(parent);
    await collectSentinel();

    expect(nodeOf(made)).toBeDefined();
  });

  it("keeps a read parsed handle of insertAdjacentHTML after a collection", async () => {
    const el = createElementNode("div");
    const made = entry("b");
    host.insertAdjacentHtml.mockImplementationOnce(() => parsed(el.__node_id, 0, [made]));

    el.insertAdjacentHTML("beforeend", "<b></b>");
    kids(el);
    await collectSentinel();

    expect(nodeOf(made)).toBeDefined();
  });

  it("keeps a handle below an old child of innerHTML while the handle of the child lives", async () => {
    const parent = createElementNode("div");
    const top = keep(createElementNode("span"));
    const inner = (() => {
      const child = createElementNode("b");
      insert(top, parent);
      insert(child, top);
      return new WeakRef(child);
    })();

    parent.innerHTML = "";
    await collectSentinel();

    expect(inner.deref()).toBeDefined();
  });

  it("lets a handle below an old child of innerHTML be collected with the child", async () => {
    const parent = createElementNode("div");
    const inner = (() => {
      const top = createElementNode("span");
      const child = createElementNode("b");
      insert(top, parent);
      insert(child, top);
      parent.innerHTML = "";
      return new WeakRef(child);
    })();

    await collectGarbage(() => inner.deref() === undefined);

    expect(inner.deref()).toBeUndefined();
  });
});
