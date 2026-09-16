// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { h, nextTick, reactive } from "@vue/runtime-core";
import { beforeEach, describe, expect, it } from "vitest";

import { createNode, rootNodeId } from "../src/index.mts";
import { createIncaApp } from "../src/vue/index.mts";
import type { NodeId } from "../src/index.mts";
import type { IncaElement } from "../src/vue/index.mts";

interface FakeNode {
  tag: string;
  attributes: Record<string, unknown>;
  style: Record<string, unknown>;
  parent: NodeId | null;
  children: NodeId[];
  listeners: Record<string, number[]>;
}

// A minimal, in-memory stand-in for the native host's retained tree — real
// enough to drive `incajs`'s actual wrapper functions and `incajs/vue`'s
// actual `nodeOps`/`patchProp` end to end, without a real Rust process.
//
// It matches the real host where the difference is observable: one parent per
// node, several callbacks per (node, event), and `destroyNode` freeing a whole
// subtree. It does not refuse a cycle the way the host does — `nodeOps` never
// builds one.
function installFakeNative(): Map<NodeId, FakeNode> {
  const nodes = new Map<NodeId, FakeNode>();
  let nextId = 0;

  function requireNode(id: NodeId): FakeNode {
    const node = nodes.get(id);
    if (!node) throw new Error(`unknown node id: ${id}`);
    return node;
  }

  function allocate(tag: string): NodeId {
    const id = nextId++;
    nodes.set(id, { tag, attributes: {}, style: {}, parent: null, children: [], listeners: {} });
    return id;
  }

  function detach(id: NodeId): void {
    const node = nodes.get(id);
    if (!node || node.parent === null) return;
    const parent = nodes.get(node.parent);
    if (parent) {
      const index = parent.children.indexOf(id);
      if (index !== -1) parent.children.splice(index, 1);
    }
    node.parent = null;
  }

  // The host allocates its root along with the tree, before any JS runs.
  const rootId = allocate("div");

  globalThis.__inca_native__ = {
    rootNodeId(): NodeId {
      return rootId;
    },
    createNode: allocate,
    appendChild(parentId: NodeId, childId: NodeId): void {
      globalThis.__inca_native__.insertBefore(parentId, childId, null);
    },
    insertBefore(parentId: NodeId, childId: NodeId, anchorId: NodeId | null): void {
      const parent = requireNode(parentId);
      const child = requireNode(childId);
      detach(childId);
      const anchorIndex = anchorId === null ? -1 : parent.children.indexOf(anchorId);
      if (anchorIndex === -1) {
        parent.children.push(childId);
      } else {
        parent.children.splice(anchorIndex, 0, childId);
      }
      child.parent = parentId;
    },
    removeChild(parentId: NodeId, childId: NodeId): void {
      requireNode(parentId);
      if (nodes.get(childId)?.parent === parentId) detach(childId);
    },
    setAttribute(nodeId: NodeId, key: string, value: unknown): void {
      requireNode(nodeId).attributes[key] = value;
    },
    setStyle(nodeId: NodeId, key: string, value: unknown): void {
      requireNode(nodeId).style[key] = value;
    },
    addEventListener(nodeId: NodeId, event: string, callbackId: number): void {
      const callbacks = (requireNode(nodeId).listeners[event] ??= []);
      if (!callbacks.includes(callbackId)) callbacks.push(callbackId);
    },
    removeEventListener(nodeId: NodeId, event: string, callbackId: number): boolean {
      const callbacks = nodes.get(nodeId)?.listeners[event];
      if (!callbacks) return false;
      const index = callbacks.indexOf(callbackId);
      if (index === -1) return false;
      callbacks.splice(index, 1);
      return true;
    },
    destroyNode(nodeId: NodeId): number[] {
      if (!nodes.has(nodeId)) return [];
      detach(nodeId);

      const released: number[] = [];
      const pending = [nodeId];
      while (pending.length > 0) {
        const current = pending.pop()!;
        const node = nodes.get(current);
        if (!node) continue;
        nodes.delete(current);
        pending.push(...node.children);
        released.push(...Object.values(node.listeners).flat());
      }
      return released;
    },
  };

  return nodes;
}

function dispatch(node: FakeNode, event: string): void {
  const callbackIds = node.listeners[event] ?? [];
  if (callbackIds.length === 0) throw new Error(`no ${event} listener registered`);
  for (const callbackId of callbackIds) {
    const callback = globalThis.__inca_callbacks__[callbackId];
    if (callback === undefined) throw new Error(`no callback registered as ${callbackId}`);
    callback();
  }
}

describe("incajs/vue renderer, driven end to end through real core internals", () => {
  let nodes: Map<NodeId, FakeNode>;
  let root: IncaElement;

  beforeEach(() => {
    nodes = installFakeNative();
    delete (globalThis as { __inca_callbacks__?: unknown }).__inca_callbacks__;
    root = { id: createNode("root"), kind: "element", parent: null, children: [] };
  });

  it("mounts styles, attributes, and text, then reacts to a click", async () => {
    const state = reactive({ count: 0 });
    const App = {
      setup() {
        return () =>
          h(
            "div",
            {
              style: { background: "#000" },
              class: "box",
              onClick: () => state.count++,
            },
            `count: ${state.count}`,
          );
      },
    };

    createIncaApp(App).mount(root);
    await nextTick();

    const rootNode = nodes.get(root.id)!;
    expect(rootNode.children).toHaveLength(1);
    const div = nodes.get(rootNode.children[0]!)!;
    expect(div.style).toEqual({ background: "#000" });
    expect(div.attributes["class"]).toBe("box");
    expect(div.children).toHaveLength(1);
    const text = nodes.get(div.children[0]!)!;
    expect(text.attributes["value"]).toBe("count: 0");

    dispatch(div, "click");
    await nextTick();

    expect(nodes.get(div.children[0]!)!.attributes["value"]).toBe("count: 1");
  });

  it("reorders a keyed list via moves, keeping each item's node identity", async () => {
    const state = reactive({ items: [1, 2, 3] });
    const App = {
      setup() {
        return () =>
          h(
            "div",
            null,
            state.items.map((n) => h("div", { key: n, class: `item-${n}` })),
          );
      },
    };

    createIncaApp(App).mount(root);
    await nextTick();

    const containerId = nodes.get(root.id)!.children[0]!;
    const originalOrder = nodes.get(containerId)!.children.slice();
    expect(originalOrder).toHaveLength(3);

    state.items = [3, 1, 2];
    await nextTick();

    const reordered = nodes.get(containerId)!.children;
    expect(reordered).toEqual([originalOrder[2], originalOrder[0], originalOrder[1]]);
  });

  it("frees a removed subtree's nodes and every callback inside it", async () => {
    const state = reactive({ shown: true });
    const App = {
      setup() {
        return () =>
          h("div", null, [state.shown ? h("div", null, [h("div", { onClick: () => {} })]) : null]);
      },
    };

    createIncaApp(App).mount(root);
    await nextTick();

    const containerId = nodes.get(root.id)!.children[0]!;
    const branchId = nodes.get(containerId)!.children[0]!;
    const leafId = nodes.get(branchId)!.children[0]!;
    expect(Object.keys(globalThis.__inca_callbacks__)).toHaveLength(1);

    state.shown = false;
    await nextTick();

    expect(nodes.has(branchId)).toBe(false);
    expect(
      nodes.has(leafId),
      "Vue removes only the subtree's root, so the descendant is freed here or nowhere",
    ).toBe(false);
    expect(globalThis.__inca_callbacks__).toEqual({});
  });

  it("keeps one native registration when a handler is replaced on every render", async () => {
    const state = reactive({ count: 0 });
    const App = {
      setup() {
        return () =>
          // A fresh closure each render, so Vue patches the prop every time.
          h("div", { onClick: () => state.count++ }, `${state.count}`);
      },
    };

    createIncaApp(App).mount(root);
    await nextTick();

    const div = nodes.get(nodes.get(root.id)!.children[0]!)!;
    dispatch(div, "click");
    await nextTick();
    dispatch(div, "click");
    await nextTick();

    expect(state.count).toBe(2);
    expect(div.listeners["click"]).toHaveLength(1);
    expect(Object.keys(globalThis.__inca_callbacks__)).toHaveLength(1);
  });

  it("unbinds a handler that goes away", async () => {
    const state = reactive({ armed: true });
    let clicks = 0;
    const App = {
      setup() {
        return () => h("div", { onClick: state.armed ? () => clicks++ : null });
      },
    };

    createIncaApp(App).mount(root);
    await nextTick();

    const div = nodes.get(nodes.get(root.id)!.children[0]!)!;
    state.armed = false;
    await nextTick();

    expect(div.listeners["click"]).toHaveLength(0);
    expect(globalThis.__inca_callbacks__).toEqual({});
    expect(clicks).toBe(0);
  });

  it("mounts against the host's root container when mount gets no argument", async () => {
    const App = {
      setup() {
        return () => h("div", { class: "box" });
      },
    };

    createIncaApp(App).mount();
    await nextTick();

    const hostRoot = nodes.get(rootNodeId())!;
    expect(hostRoot).not.toBe(nodes.get(root.id));
    expect(hostRoot.children).toHaveLength(1);
    expect(nodes.get(hostRoot.children[0]!)!.attributes["class"]).toBe("box");
  });
});
