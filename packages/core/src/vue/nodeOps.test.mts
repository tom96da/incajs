// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { elementOf, unregisterElement } from "./elements.mts";
import { createNodeOps } from "./nodeOps.mts";
import { rectOf } from "./rectOf.mts";
import type { IncaCore } from "../rendererCore.mts";
import type { EventLike, IncaEventTarget } from "../types.mts";
import type { IncaElement, IncaText } from "./nodeOps.mts";

let nextId = 1;

const add = vi.fn<IncaEventTarget["addEventListener"]>();
const remove = vi.fn<IncaEventTarget["removeEventListener"]>();
const dispatch = vi.fn<(event: EventLike) => boolean>(() => true);
const target: IncaEventTarget = {
  addEventListener: add,
  removeEventListener: remove,
  dispatchEvent: dispatch,
};

const core: IncaCore = {
  rootNodeId: vi.fn<() => number>(),
  createNode: vi.fn<(tag: string) => number>(),
  appendChild: vi.fn<(parentId: number, childId: number) => void>(),
  insertBefore: vi.fn<(parentId: number, childId: number, anchorId: number | null) => void>(),
  removeChild: vi.fn<(parentId: number, childId: number) => void>(),
  setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  getAttribute: vi.fn<(nodeId: number, key: string) => string | number | boolean | null>(
    () => null,
  ),
  removeAttribute: vi.fn<(nodeId: number, key: string) => void>(),
  setStyle: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
  removeStyle: vi.fn<(nodeId: number, key: string) => void>(),
  addEventListener: vi.fn<IncaCore["addEventListener"]>(),
  removeEventListener: vi.fn<IncaCore["removeEventListener"]>(),
  createEventTarget: vi.fn<IncaCore["createEventTarget"]>(() => target),
  latestEventId: vi.fn<() => number>(() => 0),
  destroyNode: vi.fn<(nodeId: number) => void>(),
  focus: vi.fn<(nodeId: number) => void>(),
  blur: vi.fn<(nodeId: number) => void>(),
  focusedNode: vi.fn<() => number | null>(() => null),
  layoutOf: vi.fn<
    (nodeId: number) => readonly [number, number, number, number, number, number, number] | null
  >(() => null),
  scrollOf: vi.fn<(nodeId: number) => readonly [number, number, number, number] | null>(() => null),
  scrollNode: vi.fn<(nodeId: number, left: number | null, top: number | null) => void>(),
  scrollIntoViewNode: vi.fn<(nodeId: number, block: string, inline: string) => void>(),
  click: vi.fn<(nodeId: number) => void>(),
  dispatchEvent: vi.fn<(nodeId: number, event: EventLike) => boolean>(() => true),
};

const noLayout = {
  getBoundingClientRect: () => rectOf(null),
  offsetWidth: 0,
  offsetHeight: 0,
  offsetLeft: 0,
  offsetTop: 0,
  offsetParent: null,
  clientWidth: 0,
  clientHeight: 0,
  clientLeft: 0,
  clientTop: 0,
  scrollLeft: 0,
  scrollTop: 0,
  scrollWidth: 0,
  scrollHeight: 0,
  scrollTo: () => {},
  scrollBy: () => {},
  scrollIntoView: () => {},
};

const nodeOps = createNodeOps(core);

beforeEach(() => {
  vi.clearAllMocks();
  for (let id = 1; id < nextId; id++) unregisterElement(id);
  nextId = 1;
  vi.mocked(core.createNode).mockImplementation(() => nextId++);
});

function element(): IncaElement {
  return {
    id: nextId++,
    kind: "element",
    parent: null,
    children: [],
    focus: vi.fn<() => void>(),
    blur: vi.fn<() => void>(),
    click: vi.fn<() => void>(),
    ...target,
    tabIndex: -1,
    ...noLayout,
  };
}

describe("createElement", () => {
  it("allocates a native node for the tag and returns a parentless, childless element", () => {
    const el = nodeOps.createElement("div");
    expect(core.createNode).toHaveBeenCalledWith("div");
    expect(el).toEqual({
      id: 1,
      kind: "element",
      parent: null,
      children: [],
      focus: expect.any(Function),
      blur: expect.any(Function),
      click: expect.any(Function),
      addEventListener: expect.any(Function),
      removeEventListener: expect.any(Function),
      dispatchEvent: expect.any(Function),
      tabIndex: -1,
      getBoundingClientRect: expect.any(Function),
      offsetWidth: 0,
      offsetHeight: 0,
      offsetLeft: 0,
      offsetTop: 0,
      offsetParent: null,
      clientWidth: 0,
      clientHeight: 0,
      clientLeft: 0,
      clientTop: 0,
      scrollLeft: 0,
      scrollTop: 0,
      scrollWidth: 0,
      scrollHeight: 0,
      scrollTo: expect.any(Function),
      scrollBy: expect.any(Function),
      scrollIntoView: expect.any(Function),
    });
  });
});

describe("createText", () => {
  it("creates a text node and forwards the initial value", () => {
    const node = nodeOps.createText("hello");
    expect(core.createNode).toHaveBeenCalledWith("text");
    expect(core.setAttribute).toHaveBeenCalledWith(1, "value", "hello");
    expect(node).toEqual({ id: 1, kind: "text", parent: null, text: "hello" });
  });
});

describe("createComment", () => {
  it("creates a hidden container in place of a comment", () => {
    const node = nodeOps.createComment("v-if");
    expect(core.createNode).toHaveBeenCalledWith("div");
    expect(core.setStyle).toHaveBeenCalledWith(1, "display", "none");
    expect(node).toEqual({
      id: 1,
      kind: "comment",
      parent: null,
      children: [],
      focus: expect.any(Function),
      blur: expect.any(Function),
      click: expect.any(Function),
      addEventListener: expect.any(Function),
      removeEventListener: expect.any(Function),
      dispatchEvent: expect.any(Function),
      tabIndex: -1,
      getBoundingClientRect: expect.any(Function),
      offsetWidth: 0,
      offsetHeight: 0,
      offsetLeft: 0,
      offsetTop: 0,
      offsetParent: null,
      clientWidth: 0,
      clientHeight: 0,
      clientLeft: 0,
      clientTop: 0,
      scrollLeft: 0,
      scrollTop: 0,
      scrollWidth: 0,
      scrollHeight: 0,
      scrollTo: expect.any(Function),
      scrollBy: expect.any(Function),
      scrollIntoView: expect.any(Function),
    });
  });
});

describe("focus / blur", () => {
  it("forward to core.focus/core.blur bound to the element's own id", () => {
    const el = nodeOps.createElement("div");

    el.focus();
    expect(core.focus).toHaveBeenCalledWith(el.id);

    el.blur();
    expect(core.blur).toHaveBeenCalledWith(el.id);
  });
});

describe("click", () => {
  it("forwards to core.click bound to the element's own id", () => {
    const el = nodeOps.createElement("div");

    el.click();

    expect(core.click).toHaveBeenCalledExactlyOnceWith(el.id);
  });
});

describe("event target methods", () => {
  it("come from core.createEventTarget, resolving the element's own id", () => {
    const el = nodeOps.createElement("div");
    const comment = nodeOps.createComment("v-if");

    const resolvers = vi.mocked(core.createEventTarget).mock.calls.map(([resolve]) => resolve());
    expect(resolvers).toEqual([el.id, comment.id]);
    const listener = vi.fn<() => void>();
    const event = { type: "ping" };
    dispatch.mockReturnValueOnce(false);
    el.addEventListener("click", listener, true);
    el.removeEventListener("click", listener, true);
    expect(add).toHaveBeenCalledExactlyOnceWith("click", listener, true);
    expect(remove).toHaveBeenCalledExactlyOnceWith("click", listener, true);
    expect(el.dispatchEvent(event)).toBe(false);
    expect(dispatch).toHaveBeenCalledExactlyOnceWith(event);
  });
});

describe("setText", () => {
  it("forwards the new value and updates the node", () => {
    const node = nodeOps.createText("old") as IncaText;
    nodeOps.setText(node, "new");
    expect(core.setAttribute).toHaveBeenCalledWith(1, "value", "new");
    expect(node.text).toBe("new");
  });
});

describe("setElementText", () => {
  it("replaces existing children with one text child", () => {
    const el = element();
    const oldChild = element();
    el.children.push(oldChild);
    oldChild.parent = el;

    nodeOps.setElementText(el, "hello");

    expect(core.destroyNode).toHaveBeenCalledWith(oldChild.id);
    expect(oldChild.parent).toBeNull();
    expect(el.children).toHaveLength(1);
    expect(el.children[0]).toMatchObject({ kind: "text", text: "hello" });
  });

  it("leaves no children behind for an empty string", () => {
    const el = element();
    const oldChild = element();
    el.children.push(oldChild);
    oldChild.parent = el;

    nodeOps.setElementText(el, "");

    expect(el.children).toHaveLength(0);
  });
});

describe("insert", () => {
  it("appends when no anchor is given", () => {
    const parent = element();
    const child = element();

    nodeOps.insert(child, parent);

    expect(core.insertBefore).toHaveBeenCalledWith(parent.id, child.id, null);
    expect(parent.children).toEqual([child]);
    expect(child.parent).toBe(parent);
  });

  it("inserts before an existing anchor child", () => {
    const parent = element();
    const first = element();
    const anchor = element();
    nodeOps.insert(first, parent);
    nodeOps.insert(anchor, parent);
    vi.mocked(core.insertBefore).mockClear();

    const inserted = element();
    nodeOps.insert(inserted, parent, anchor);

    expect(core.insertBefore).toHaveBeenCalledWith(parent.id, inserted.id, anchor.id);
    expect(parent.children).toEqual([first, inserted, anchor]);
  });

  it("leaves a child alone when it is its own anchor", () => {
    const parent = element();
    const first = element();
    const second = element();
    nodeOps.insert(first, parent);
    nodeOps.insert(second, parent);
    vi.clearAllMocks();

    nodeOps.insert(first, parent, first);

    expect(core.removeChild).not.toHaveBeenCalled();
    expect(core.insertBefore).not.toHaveBeenCalled();
    expect(parent.children).toEqual([first, second]);
  });

  it("falls back to appending when the anchor isn't currently a child", () => {
    const parent = element();
    const strayAnchor = element();
    const child = element();

    nodeOps.insert(child, parent, strayAnchor);

    expect(core.insertBefore).toHaveBeenCalledWith(parent.id, child.id, null);
    expect(parent.children).toEqual([child]);
  });

  it("moves a child already attached elsewhere, detaching it from its old parent first", () => {
    const oldParent = element();
    const newParent = element();
    const child = element();
    nodeOps.insert(child, oldParent);

    nodeOps.insert(child, newParent);

    expect(core.removeChild).toHaveBeenCalledWith(oldParent.id, child.id);
    expect(core.destroyNode).not.toHaveBeenCalled();
    expect(oldParent.children).toHaveLength(0);
    expect(newParent.children).toEqual([child]);
    expect(child.parent).toBe(newParent);
  });
});

describe("remove", () => {
  it("frees the node and unlinks it from its parent", () => {
    const parent = element();
    const child = element();
    nodeOps.insert(child, parent);
    vi.mocked(core.removeChild).mockClear();

    nodeOps.remove(child);

    expect(core.destroyNode).toHaveBeenCalledWith(child.id);
    expect(
      core.removeChild,
      "destroyNode detaches natively, so a second call would be redundant",
    ).not.toHaveBeenCalled();
    expect(parent.children).toHaveLength(0);
    expect(child.parent).toBeNull();
  });

  it("turns the handles of the removed subtree into inert targets", () => {
    const root = nodeOps.createElement("div");
    const mid = nodeOps.createElement("div");
    const leaf = nodeOps.createElement("button");
    nodeOps.insert(nodeOps.createText("t"), mid);
    nodeOps.insert(leaf, mid);
    nodeOps.insert(mid, root);
    vi.mocked(core.setAttribute).mockClear();

    nodeOps.remove(root);

    for (const el of [root, leaf]) {
      el.addEventListener("click", vi.fn<() => void>());
      el.removeEventListener("click", vi.fn<() => void>());
      el.tabIndex = 5;
    }
    expect(root.dispatchEvent({ type: "ping" })).toBe(true);
    expect([root.tabIndex, leaf.tabIndex]).toEqual([-1, 0]);
    expect(add).not.toHaveBeenCalled();
    expect(remove).not.toHaveBeenCalled();
    expect(dispatch).not.toHaveBeenCalled();
    expect(core.getAttribute).not.toHaveBeenCalled();
    expect(core.setAttribute).not.toHaveBeenCalled();
  });

  it("turns the children setElementText frees into inert targets", () => {
    const root = nodeOps.createElement("div");
    const child = nodeOps.createElement("div");
    nodeOps.insert(child, root);

    nodeOps.setElementText(root, "");
    child.addEventListener("click", vi.fn<() => void>());

    expect(add).not.toHaveBeenCalled();
    expect(child.dispatchEvent({ type: "ping" })).toBe(true);
  });

  it("frees a node that was never attached", () => {
    const child = element();

    nodeOps.remove(child);

    expect(core.destroyNode).toHaveBeenCalledWith(child.id);
    expect(core.removeChild).not.toHaveBeenCalled();
  });
});

describe("element registry", () => {
  it("drops the whole removed subtree", () => {
    const root = nodeOps.createElement("div");
    const mid = nodeOps.createElement("div");
    const leaf = nodeOps.createElement("button");
    nodeOps.insert(leaf, mid);
    nodeOps.insert(mid, root);
    expect([root, mid, leaf].map((el) => elementOf(el.id))).toEqual([root, mid, leaf]);

    nodeOps.remove(root);

    expect([root, mid, leaf].map((el) => elementOf(el.id) !== undefined)).toEqual([
      false,
      false,
      false,
    ]);
  });

  it("drops the children setElementText replaces, and their descendants", () => {
    const root = nodeOps.createElement("div");
    const child = nodeOps.createElement("div");
    const grandchild = nodeOps.createElement("div");
    nodeOps.insert(grandchild, child);
    nodeOps.insert(child, root);

    nodeOps.setElementText(root, "hello");

    expect(elementOf(child.id) !== undefined).toBe(false);
    expect(elementOf(grandchild.id) !== undefined).toBe(false);
    expect(elementOf(root.id)).toBe(root);
  });
});

describe("parentNode / nextSibling", () => {
  it("read back what insert recorded", () => {
    const parent = element();
    const first = element();
    const second = element();
    nodeOps.insert(first, parent);
    nodeOps.insert(second, parent);

    expect(nodeOps.parentNode(first)).toBe(parent);
    expect(nodeOps.nextSibling(first)).toBe(second);
    expect(nodeOps.nextSibling(second)).toBeNull();
  });

  it("return null for a node with no parent", () => {
    const detached = element();

    expect(nodeOps.parentNode(detached)).toBeNull();
    expect(nodeOps.nextSibling(detached)).toBeNull();
  });
});
