// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { adoptFragment } from "../../tests/adopt.mts";
import { installFakeHost } from "../../tests/fakeHost.mts";
import { collectGarbage, collectSentinel } from "../../tests/gc.mts";
import { addCallback, callbackIdFor, groupOf } from "../callbacks.mts";
import {
  createCommentNode,
  createElementNode,
  createTextNode,
  documentHandle,
  rootElement,
} from "./handle.mts";
import { isRemoved, markFreed, nodeOf, releaseWhenCollected } from "./lifecycle.mts";
import * as tree from "./treeOps.mts";
import type { FakeHost } from "../../tests/fakeHost.mts";
import type { Node } from "./nodeTypes.mts";
import type { Element } from "./types.mts";

let host: FakeHost;

beforeEach(() => {
  vi.clearAllMocks();
  host = installFakeHost();
});

/** The children of `node` as the host lists them. */
const kids = (node: Node): Node[] => Array.from(node.childNodes);
const div = () => createElementNode("div");

describe("insert", () => {
  it("appends when no anchor is given", () => {
    const parent = div();
    const child = div();

    tree.insert(child, parent);

    expect(host.spies.insertBefore).toHaveBeenCalledWith(parent.__node_id, child.__node_id, null);
    expect(kids(parent).map((node) => node.__node_id)).toEqual(
      [child].map((node) => node.__node_id),
    );
  });

  it("passes the id of the anchor to the host", () => {
    const parent = div();
    const first = div();
    const anchor = div();
    tree.insert(first, parent);
    tree.insert(anchor, parent);

    const inserted = div();
    tree.insert(inserted, parent, anchor);

    expect(host.spies.insertBefore).toHaveBeenLastCalledWith(
      parent.__node_id,
      inserted.__node_id,
      anchor.__node_id,
    );
    expect(kids(parent).map((node) => node.__node_id)).toEqual(
      [first, inserted, anchor].map((node) => node.__node_id),
    );
  });

  it("leaves a child alone when it is its own anchor", () => {
    const parent = div();
    const first = div();
    const second = div();
    tree.insert(first, parent);
    tree.insert(second, parent);

    tree.insert(first, parent, first);

    expect(kids(parent).map((node) => node.__node_id)).toEqual(
      [first, second].map((node) => node.__node_id),
    );
  });

  it("throws NotFoundError for an anchor that is no child", () => {
    const parent = div();

    expect(() => tree.insert(div(), parent, div())).toThrow(
      expect.objectContaining({ name: "NotFoundError" }),
    );
  });

  it("throws NotFoundError for a freed anchor without a host call", () => {
    const parent = div();
    const anchor = div();
    tree.insert(anchor, parent);
    host.nodes.delete(anchor.__node_id);
    markFreed([anchor.__node_id]);
    host.spies.insertBefore.mockClear();

    expect(() => tree.insert(div(), parent, anchor)).toThrow(
      expect.objectContaining({ name: "NotFoundError" }),
    );
    expect(host.spies.insertBefore).not.toHaveBeenCalled();
  });

  it("moves a child already attached elsewhere", () => {
    const oldParent = div();
    const newParent = div();
    const child = div();
    tree.insert(child, oldParent);

    tree.insert(child, newParent);

    expect(kids(oldParent)).toEqual([]);
    expect(kids(newParent).map((node) => node.__node_id)).toEqual([child.__node_id]);
  });

  it("lets the handle of an inserted fragment be collected", async () => {
    const parent = div();
    const group = (() => {
      const made = adoptFragment(host.create("#fragment"));
      tree.insert(made, parent);
      return new WeakRef(made);
    })();

    await collectGarbage(() => group.deref() === undefined);

    expect(group.deref()).toBeUndefined();
  });
});

describe("remove", () => {
  it("takes the node out of its parent", () => {
    const parent = div();
    const child = div();
    tree.insert(child, parent);

    tree.remove(child);

    expect(kids(parent).map((node) => node.__node_id)).toEqual([]);
  });

  it("keeps the removed node in the host", () => {
    const child = div();
    tree.insert(child, div());

    tree.remove(child);

    expect(host.nodes.has(child.__node_id)).toBe(true);
  });

  it("makes one removeNode call with the id of the node", () => {
    const parent = div();
    const child = div();
    tree.insert(child, parent);

    tree.remove(child);

    expect(host.spies.removeNode.mock.calls).toEqual([[child.__node_id]]);
  });

  it("makes no holdNode call for a node that is not held", () => {
    const child = div();

    tree.remove(child);

    expect(host.spies.holdNode).not.toHaveBeenCalled();
  });

  it("forgets the callback ids the host reports", () => {
    const [releasedListener, keptListener] = [vi.fn<() => void>(), vi.fn<() => void>()];
    const child = div();
    addCallback(groupOf(child.__node_id, "click", false), releasedListener, 501);
    addCallback(groupOf(host.ROOT, "click", false), keptListener, 502);
    host.spies.removeNode.mockReturnValueOnce([[501], [child.__node_id]]);

    tree.remove(child);

    expect(
      callbackIdFor(groupOf(child.__node_id, "click", false), releasedListener),
    ).toBeUndefined();
    expect(callbackIdFor(groupOf(host.ROOT, "click", false), keptListener)).toBe(502);
  });

  it("marks a descendant removed through the node ids the host returns", () => {
    const root = div();
    const leaf = div();
    tree.insert(leaf, root);

    tree.remove(root);

    expect(isRemoved(leaf)).toBe(true);
  });

  it("marks the content of a template removed", () => {
    const owner = createElementNode("template") as unknown as Element & {
      content: Node;
    };

    tree.remove(owner);

    expect(isRemoved(owner.content)).toBe(true);
  });

  it("changes nothing for a freed node", () => {
    const child = div();
    markFreed([child.__node_id]);

    tree.remove(child);

    expect(host.spies.removeNode).not.toHaveBeenCalled();
  });

  it("releases a removed node that is held already once when its handle is collected", async () => {
    const id = (() => {
      const child = div();
      releaseWhenCollected(child);
      tree.remove(child);
      return child.__node_id;
    })();

    await collectGarbage(() => host.spies.releaseNode.mock.calls.some(([n]) => n === id));
    await collectSentinel();

    expect(host.spies.releaseNode.mock.calls.filter(([n]) => n === id)).toHaveLength(1);
  });

  it("skips an id of the removed subtree that has no handle", () => {
    const root = div();
    tree.insert(div(), root);
    host.natives.appendChild!(root.__node_id, host.create("span"));

    tree.remove(root);

    expect(isRemoved(root)).toBe(true);
  });

  it.each([
    ["an element", () => div()],
    ["a text leaf", () => createTextNode("t")],
    ["a comment", () => createCommentNode()],
  ])("removes %s with no parent", (_name, create) => {
    const node = create();

    tree.remove(node);

    expect(isRemoved(node)).toBe(true);
  });

  it("makes the removed subtree ignore listener calls", () => {
    const add = vi.fn<() => number>(() => 0);
    const remove = vi.fn<() => boolean>(() => true);
    host = installFakeHost({ addEventListener: add, removeEventListener: remove });
    const root = div();
    const mid = div();
    const leaf = createElementNode("button");
    tree.insert(createTextNode("t"), mid);
    tree.insert(leaf, mid);
    tree.insert(mid, root);

    tree.remove(root);

    for (const el of [root, leaf]) {
      el.addEventListener("click", vi.fn<() => void>());
      el.removeEventListener("click", vi.fn<() => void>());
    }
    expect(add).not.toHaveBeenCalled();
    expect(remove).not.toHaveBeenCalled();
  });

  it("keeps the attributes of a descendant readable", () => {
    host = installFakeHost({ getAttribute: vi.fn<() => string>(() => "7") as never });
    const root = div();
    const leaf = createElementNode("button");
    tree.insert(leaf, root);

    tree.remove(root);

    expect([root.id, leaf.id, leaf.tabIndex]).toEqual(["7", "7", 7]);
  });
});

describe("parentNode and nextSibling", () => {
  it("read the links the host holds", () => {
    const parent = div();
    const first = div();
    const second = div();
    tree.insert(first, parent);
    tree.insert(second, parent);

    expect(tree.parentNode(first)).toBe(parent);
    expect(tree.nextSibling(first)).toBe(second);
    expect(tree.nextSibling(second)).toBeNull();
  });

  it.each([
    ["a node with no parent", () => div()],
    [
      "a node in a fragment",
      () => {
        const group = adoptFragment(host.create("#fragment"));
        const child = div();
        group.appendChild(child);
        return child;
      },
    ],
  ])("parentNode gives null for %s", (_name, create) => {
    expect(tree.parentNode(create())).toBeNull();
  });

  it("parentNode gives null for a child of the document", () => {
    const comment = createCommentNode();
    host.natives.insertBefore!(host.DOCUMENT, comment.__node_id, host.ROOT);

    expect(tree.parentNode(comment)).toBeNull();
  });

  it("nextSibling gives a text or comment sibling", () => {
    const parent = div();
    const first = div();
    const text = createTextNode("t");
    const comment = createCommentNode();
    for (const child of [first, text, comment]) tree.insert(child, parent);

    expect(tree.nextSibling(first)).toBe(text);
    expect(tree.nextSibling(text)).toBe(comment);
  });

  it("nextSibling gives null for a node with no parent", () => {
    expect(tree.nextSibling(div())).toBeNull();
  });

  it("answer null for a freed node", () => {
    const parent = div();
    const child = div();
    tree.insert(child, parent);
    markFreed([child.__node_id]);

    expect([tree.parentNode(child), tree.nextSibling(child)]).toEqual([null, null]);
  });

  it("make no host call for a freed node", () => {
    const parent = div();
    const child = div();
    tree.insert(child, parent);
    markFreed([child.__node_id]);
    host.spies.linkedNode.mockClear();

    tree.parentNode(child);
    tree.nextSibling(child);

    expect(host.spies.linkedNode).not.toHaveBeenCalled();
  });

  it("nextSibling builds the handle of a sibling that has none, after one link read", () => {
    const parent = div();
    const first = div();
    tree.insert(first, parent);
    host.natives.appendChild!(parent.__node_id, host.create("span"));
    host.spies.linkedNode.mockClear();

    expect(tree.nextSibling(first)).toBeTruthy();
    expect(host.spies.linkedNode).toHaveBeenCalledTimes(1);
  });
});

describe("the handles of the table", () => {
  /** Builds the objects `build` returns and drops every reference to them. */
  const weakly = <T extends object>(build: () => T): WeakRef<T> => new WeakRef(build());

  it("keeps the handle of an inserted node while it is attached", async () => {
    const parent = div();
    const [handle, id] = (() => {
      const child = div();
      tree.insert(child, parent);
      return [new WeakRef(child), child.__node_id];
    })();

    await collectSentinel();

    expect(handle.deref()).toBeDefined();
    expect(nodeOf(id)).toBe(handle.deref());
  });

  it("lets the handle of a removed node be collected", async () => {
    const handle = weakly(() => {
      const child = div();
      tree.insert(child, div());
      tree.remove(child);
      return child;
    });

    await collectGarbage(() => handle.deref() === undefined);

    expect(handle.deref()).toBeUndefined();
  });

  it("releases the hold of a removed node once its handle is collected", async () => {
    const id = (() => {
      const child = div();
      tree.remove(child);
      return child.__node_id;
    })();

    await collectGarbage(() => host.spies.releaseNode.mock.calls.some(([n]) => n === id));

    expect(host.spies.releaseNode).toHaveBeenCalledWith(id);
  });

  it("keeps the handle of a descendant of a removed node", async () => {
    const root = div();
    const leaf = weakly(() => {
      const child = div();
      tree.insert(child, root);
      return child;
    });
    tree.remove(root);

    await collectSentinel();

    expect(root.firstChild).toBe(leaf.deref());
    expect(leaf.deref()).toBeDefined();
  });

  it("lets a descendant's handle be collected with the removed node before a frame", async () => {
    const leaf = weakly(() => {
      const root = div();
      const child = div();
      tree.insert(child, root);
      tree.remove(root);
      return child;
    });

    await collectGarbage(() => leaf.deref() === undefined);

    expect(leaf.deref()).toBeUndefined();
  });

  it("keeps the handle of a descendant through a second removal of the node", async () => {
    const root = div();
    const leaf = weakly(() => {
      const child = div();
      tree.insert(child, root);
      return child;
    });
    tree.remove(root);
    tree.insert(root, div());
    tree.remove(root);

    await collectSentinel();

    expect(root.firstChild).toBe(leaf.deref());
    expect(leaf.deref()).toBeDefined();
  });

  it("keeps the descendants of a reinserted branch while its root is attached", async () => {
    const root = div();
    const parent = div();
    const leaf = weakly(() => {
      const child = div();
      const grandchild = div();
      tree.insert(child, root);
      tree.insert(grandchild, child);
      (grandchild as unknown as { mark: string }).mark = "kept";
      return grandchild;
    });
    tree.remove(root);
    tree.insert(root, parent);

    await collectSentinel();

    expect(leaf.deref()).toBeDefined();
    const grandchild = root.firstChild?.firstChild as unknown as { mark?: string } | null;
    expect(grandchild?.mark).toBe("kept");
  });

  it("keeps the handle of a descendant of a node removed out of a removed subtree", async () => {
    const mid = div();
    const [top, leaf] = (() => {
      const root = div();
      const child = div();
      tree.insert(child, mid);
      tree.insert(mid, root);
      tree.remove(root);
      tree.remove(mid);
      return [new WeakRef(root), new WeakRef(child)];
    })();

    await collectGarbage(() => top.deref() === undefined);

    expect(top.deref()).toBeUndefined();
    expect(mid.firstChild).toBe(leaf.deref());
    expect(leaf.deref()).toBeDefined();
  });
});

describe("the mount container", () => {
  it("is the next sibling of a node the script put before it in the document", () => {
    const comment = createCommentNode();

    documentHandle().insertBefore(comment, rootElement());

    expect(tree.nextSibling(comment)).toBe(rootElement());
  });

  it("is collectable after the script put it back under the document", async () => {
    const container = (() => {
      const root = rootElement();
      documentHandle().appendChild(root);
      return new WeakRef(root);
    })();

    await collectGarbage(() => container.deref() === undefined);

    expect(container.deref()).toBeUndefined();
  });
});
