// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { createElementNode } from "./handle.mts";
import { markDestroyed } from "./lifecycle.mts";
import { insert } from "./shadowTree.mts";
import type { Parsed, ParsedEntry } from "../native.mts";
import type { IncaElement, IncaTemplateElement, IncaText } from "./types.mts";

let templates = new Set<number>();

const host = {
  createNode: vi.fn<(tag: string) => number>(() => 1),
  templateContent: vi.fn<(nodeId: number) => number | null>((id) =>
    templates.has(id) ? id + 1 : null,
  ),
  appendChild: vi.fn<() => number>(() => 0),
  insertBefore: vi.fn<() => number>(() => 0),
  removeChild: vi.fn<() => void>(),
  setAttribute: vi.fn<() => void>(),
  getAttribute: vi.fn<() => null>(() => null),
  setStyle: vi.fn<() => void>(),
  holdNode: vi.fn<(nodeId: number) => void>(),
  releaseNode: vi.fn<(nodeId: number) => void>(),
  releaseListeners: vi.fn<(nodeId: number) => number[]>(() => []),
  innerHtml: vi.fn<(nodeId: number) => string>(() => "<i></i>"),
  outerHtml: vi.fn<(nodeId: number) => string>(() => "<p><i></i></p>"),
  setInnerHtml: vi.fn<(nodeId: number, html: string) => Parsed>(() => [0, []]),
  setOuterHtml: vi.fn<(nodeId: number, html: string) => Parsed | null>(() => [0, []]),
  insertAdjacentHtml: vi.fn<(nodeId: number, position: string, html: string) => Parsed>(() => [
    0,
    [],
  ]),
};

beforeEach(() => {
  vi.clearAllMocks();
  templates = new Set();
  globalThis.__inca_native__ = host as never;
});

let nextId = 500;
const withChild = (): [IncaElement, IncaElement] => {
  const parent = createElementNode("div");
  const child = createElementNode("span");
  insert(child, parent);
  return [parent, child];
};
const entry = (tag: string, parentId = 0, data: string | null = null): ParsedEntry => [
  nextId++,
  tag,
  parentId,
  data,
];

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
  ] as const)("links the nodes of %s into %s", (position, _into, target) => {
    const [parent, child] = withChild();
    const made = entry("b");
    host.insertAdjacentHtml.mockReturnValueOnce([1, [made]]);

    child.insertAdjacentHTML(position, "<b>");

    const container = [parent, child][target]!;
    expect(container.__children.map((node) => node.__node_id)).toContain(made[0]);
  });
});

describe("the shadow tree after a replacement", () => {
  it("links the roots of innerHTML at the returned position", () => {
    const [parent] = withChild();
    const first = entry("b");
    const second = entry("i");
    host.setInnerHtml.mockReturnValueOnce([0, [first, second]]);

    parent.innerHTML = "<b></b><i></i>";

    expect(parent.__children.map((node) => node.__node_id)).toEqual([first[0], second[0]]);
  });

  it("unlinks the old children of innerHTML", () => {
    const [parent, old] = withChild();

    parent.innerHTML = "";

    expect([parent.__children.length, old.parent]).toEqual([0, null]);
  });

  it("keeps the listeners of an old child of innerHTML", () => {
    const [parent] = withChild();

    parent.innerHTML = "";

    expect(host.releaseListeners).not.toHaveBeenCalled();
  });

  it("holds an old child of innerHTML in the host", () => {
    const [parent, old] = withChild();

    parent.innerHTML = "";

    expect(host.holdNode).toHaveBeenCalledWith(old.__node_id);
  });

  it("replaces the element of outerHTML by its nodes at the returned position", () => {
    const parent = createElementNode("div");
    const before = createElementNode("a");
    const target = createElementNode("b");
    insert(before, parent);
    insert(target, parent);
    const made = entry("i");
    host.setOuterHtml.mockReturnValueOnce([1, [made]]);

    target.outerHTML = "<i></i>";

    expect(parent.__children.map((node) => node.__node_id)).toEqual([before.__node_id, made[0]]);
  });

  it("leaves the old element of outerHTML parentless", () => {
    const [, child] = withChild();

    child.outerHTML = "";

    expect(child.parent).toBeNull();
  });

  it("keeps the listeners of the old element of outerHTML", () => {
    const [, child] = withChild();

    child.outerHTML = "";

    expect(host.releaseListeners).not.toHaveBeenCalled();
  });

  it("holds the old element of outerHTML in the host", () => {
    const [, child] = withChild();

    child.outerHTML = "";

    expect(host.holdNode).toHaveBeenCalledWith(child.__node_id);
  });

  it("changes nothing for an outerHTML the host declines", () => {
    const [parent, child] = withChild();
    host.setOuterHtml.mockReturnValueOnce(null);

    child.outerHTML = "<i></i>";

    expect(parent.__children).toEqual([child]);
  });
});

describe("adopt", () => {
  it("links the nested entries by parent id", () => {
    const el = createElementNode("div");
    const outer = entry("p");
    const inner = entry("b", outer[0]);
    const text = entry("#text", inner[0], "t");
    host.setInnerHtml.mockReturnValueOnce([0, [outer, inner, text]]);

    el.innerHTML = "<p><b>t</b></p>";

    const p = el.__children[0] as IncaElement;
    const b = p.__children[0] as IncaElement;
    expect([el.__children.length, b.tagName, (b.__children[0] as IncaText).text]).toEqual([
      1,
      "B",
      "t",
    ]);
  });

  it("reaches the content of a parsed template", () => {
    const el = createElementNode("div");
    const template: ParsedEntry = [600, "template", 0, null];
    const content: ParsedEntry = [601, "#fragment", 600, null];
    const child: ParsedEntry = [602, "p", 601, null];
    templates.add(600);
    host.setInnerHtml.mockReturnValueOnce([0, [template, content, child]]);

    el.innerHTML = "<template><p></p></template>";

    const made = el.__children[0] as IncaTemplateElement;
    expect([made.__children.length, made.content.__children[0]?.__node_id]).toEqual([0, 602]);
  });

  it("makes a comment stand-in without the markup members", () => {
    const el = createElementNode("div");
    host.setInnerHtml.mockReturnValueOnce([0, [entry("#comment", 0, "c")]]);

    el.innerHTML = "<!--c-->";

    const comment = el.__children[0] as IncaElement;
    expect([comment.kind, "innerHTML" in comment, "insertAdjacentHTML" in comment]).toEqual([
      "comment",
      false,
      false,
    ]);
  });
});

describe("reads", () => {
  it.each([
    ["innerHTML", "<i></i>"],
    ["outerHTML", "<p><i></i></p>"],
  ] as const)("%s asks the host", (member, markup) => {
    const el = createElementNode("div");

    expect(el[member]).toBe(markup);
  });
});

describe("a freed element", () => {
  it.each(["innerHTML", "outerHTML"] as const)("reads an empty %s", (member) => {
    const el = createElementNode("div");
    markDestroyed(el);

    expect(el[member]).toBe("");
  });

  it.each([
    [
      "innerHTML",
      (el: IncaElement): void => void (el.innerHTML = "<b></b>"),
      () => host.setInnerHtml,
    ],
    [
      "outerHTML",
      (el: IncaElement): void => void (el.outerHTML = "<b></b>"),
      () => host.setOuterHtml,
    ],
    [
      "insertAdjacentHTML",
      (el: IncaElement) => el.insertAdjacentHTML("beforeend", "<b></b>"),
      () => host.insertAdjacentHtml,
    ],
  ] as const)("ignores the assignment of %s", (_member, assign, call) => {
    const el = createElementNode("div");
    markDestroyed(el);

    assign(el);

    expect(call()).not.toHaveBeenCalled();
  });
});
