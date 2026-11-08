// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import { createCommentNode, createElementNode, createTextNode, rootElement } from "./handle.mts";
import { insert, remove } from "./shadowTree.mts";

let nextId = 1;

beforeEach(() => {
  nextId = 1;
  globalThis.__inca_native__ = nativeStub({
    rootNodeId: () => 0,
    createNode: vi.fn<() => number>(() => nextId++),
    setAttribute: vi.fn<() => void>(),
    setStyle: vi.fn<() => void>(),
    insertBefore: vi.fn<() => void>(),
    removeChild: vi.fn<() => void>(),
    releaseListeners: vi.fn<() => number[]>(() => []),
    holdNode: vi.fn<() => void>(),
    releaseNode: vi.fn<() => void>(),
    attributeNames: vi.fn<() => string[]>(() => []),
  } as never);
});

/** A `div` holding a text leaf, a comment stand-in and two elements. */
function mixed() {
  const parent = createElementNode("div");
  const text = createTextNode("t");
  const comment = createCommentNode();
  const first = createElementNode("span");
  const second = createElementNode("button");
  for (const child of [text, comment, first, second]) insert(child, parent);
  return { parent, text, comment, first, second };
}

describe("parentElement", () => {
  it("is the parent", () => {
    const parent = createElementNode("div");
    const child = createElementNode("div");
    insert(child, parent);

    expect(child.parentElement).toBe(parent);
  });

  it("is null for a detached node", () => {
    expect(createElementNode("div").parentElement).toBeNull();
  });

  it("is the mount container for a top-level element", () => {
    const container = rootElement();
    const top = createElementNode("div");
    insert(top, container);

    expect(top.parentElement).toBe(container);
  });

  it("is null for the mount container", () => {
    expect(rootElement().parentElement).toBeNull();
  });
});

describe("element siblings", () => {
  it("nextElementSibling skips a text leaf and a comment stand-in", () => {
    const { parent, first, second } = mixed();
    const lead = createElementNode("div");
    insert(lead, parent, parent.__children[0]);

    expect(lead.nextElementSibling).toBe(first);
    expect(first.nextElementSibling).toBe(second);
    expect(second.nextElementSibling).toBeNull();
  });

  it("previousElementSibling is null for the first element", () => {
    const { first, second } = mixed();

    expect(first.previousElementSibling).toBeNull();
    expect(second.previousElementSibling).toBe(first);
  });

  it("are null for a detached node", () => {
    const lone = createElementNode("div");

    expect([lone.previousElementSibling, lone.nextElementSibling]).toEqual([null, null]);
  });
});

describe("children", () => {
  it("lists the elements in tree order", () => {
    const { parent, first, second } = mixed();

    expect(Array.from(parent.children)).toEqual([first, second]);
  });

  it("skips a text leaf and a comment stand-in", () => {
    const { parent } = mixed();

    expect(parent.children.length).toBe(2);
  });

  it("has an empty collection on a comment stand-in", () => {
    expect(createCommentNode().children.length).toBe(0);
  });

  it("returns the same object on every read", () => {
    const { parent } = mixed();

    expect(parent.children).toBe(parent.children);
  });

  it("follows an append", () => {
    const { parent, first, second } = mixed();
    const earlier = parent.children;
    const added = createElementNode("div");

    insert(added, parent);

    expect(Array.from(earlier)).toEqual([first, second, added]);
  });

  it("follows a removal", () => {
    const { parent, first, second } = mixed();
    const earlier = parent.children;

    remove(first);

    expect(Array.from(earlier)).toEqual([second]);
  });

  it("keeps listing the element children of a removed element", () => {
    const { parent, first, second } = mixed();
    const holder = createElementNode("div");
    insert(parent, holder);

    remove(parent);

    expect(Array.from(parent.children)).toEqual([first, second]);
  });
});

describe("element children", () => {
  it("skip text leaves and comment stand-ins", () => {
    const { parent, first, second } = mixed();

    expect(parent.firstElementChild).toBe(first);
    expect(parent.lastElementChild).toBe(second);
    expect(parent.childElementCount).toBe(2);
  });

  it("are null and zero without an element child", () => {
    const parent = createElementNode("div");
    insert(createTextNode("t"), parent);

    expect([parent.firstElementChild, parent.lastElementChild, parent.childElementCount]).toEqual([
      null,
      null,
      0,
    ]);
  });
});

describe("the tree members", () => {
  it("follow an insert", () => {
    const { parent } = mixed();
    const added = createElementNode("div");

    insert(added, parent);

    expect([parent.lastElementChild, parent.childElementCount]).toEqual([added, 3]);
  });

  it("follow a remove", () => {
    const { parent, first, second } = mixed();

    remove(first);

    expect([parent.firstElementChild, parent.childElementCount]).toEqual([second, 1]);
    expect(first.parentElement).toBeNull();
  });
});
