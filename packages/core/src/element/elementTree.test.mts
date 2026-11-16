// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it } from "vitest";

import { installFakeHost } from "../../tests/fakeHost.mts";
import { createCommentNode, createElementNode, createTextNode, rootElement } from "./handle.mts";
import { markFreed } from "./lifecycle.mts";
import { insert, remove } from "./treeOps.mts";
import type { FakeHost } from "../../tests/fakeHost.mts";
import type { IncaElement } from "./types.mts";

let host: FakeHost;

beforeEach(() => {
  host = installFakeHost();
});

/** A `div` holding a text leaf, a comment and two elements. */
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
  it("nextElementSibling skips a text leaf and a comment", () => {
    const { parent, first, second } = mixed();
    const lead = createElementNode("div");
    insert(lead, parent, parent.firstChild);

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

  it("skips a text leaf and a comment", () => {
    const { parent } = mixed();

    expect(parent.children.length).toBe(2);
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
  it("skip text leaves and comments", () => {
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

describe("the element flag", () => {
  it.each([
    ["previousElementSibling", "previous"],
    ["nextElementSibling", "next"],
  ] as const)("%s asks the host for the nearest element", (member, link) => {
    const { first } = mixed();

    void first[member];

    expect(host.spies.linkedNode).toHaveBeenCalledWith(first.__node_id, link, true);
  });
});

describe("a freed element", () => {
  it("reads no element link", () => {
    const { first } = mixed();
    markFreed([first.__node_id]);

    expect([
      first.parentElement,
      first.previousElementSibling,
      first.nextElementSibling,
      first.firstElementChild,
      first.lastElementChild,
    ]).toEqual([null, null, null, null, null]);
  });

  it("lists no child", () => {
    const { parent } = mixed();
    markFreed([parent.__node_id]);

    expect([parent.children.length, parent.childElementCount]).toEqual([0, 0]);
  });
});

describe("a child with no handle", () => {
  /** A `div` whose host child came from a raw native call, beside a child with a handle. */
  function withRawChild(): IncaElement {
    const parent = createElementNode("div");
    host.natives.appendChild!(parent.__node_id, host.create("span"));
    insert(createElementNode("button"), parent);
    return parent;
  }

  it.each([
    ["children", (parent: IncaElement) => parent.children.length],
    ["childNodes", (parent: IncaElement) => parent.childNodes.length],
    ["childElementCount", (parent: IncaElement) => parent.childElementCount],
  ] as const)("is skipped by %s", (_member, count) => {
    expect(count(withRawChild())).toBe(1);
  });
});
