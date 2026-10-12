// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { h, nextTick, reactive, ref } from "@vue/runtime-core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { createEventTarget } from "../src/createEventTarget.mts";
import { rectOf } from "../src/element/geometry.mts";
import { blur, click, focus } from "../src/events.mts";
import { createNode, rootNodeId } from "../src/tree.mts";
import { createIncaApp } from "../src/vue/index.mts";
import type { IncaElement, NodeId } from "../src/index.mts";

/**
 * What the fake host knows of each registration beyond its node and event.
 */
const phases = new Map<number, { capture: boolean; passive: boolean }>();

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
    getAttribute(nodeId: NodeId, key: string): string | number | boolean | null {
      return (requireNode(nodeId).attributes[key] ?? null) as string | number | boolean | null;
    },
    layoutOf(
      _nodeId: NodeId,
    ): readonly [number, number, number, number, number, number, number] | null {
      return null;
    },
    scrollOf(_nodeId: NodeId): readonly [number, number, number, number] | null {
      return null;
    },
    scrollNode(_nodeId: NodeId, _left: number | null, _top: number | null): void {},
    scrollIntoViewNode(_nodeId: NodeId, _block: string, _inline: string): void {},
    removeAttribute(nodeId: NodeId, key: string): void {
      delete requireNode(nodeId).attributes[key];
    },
    setStyle(nodeId: NodeId, key: string, value: unknown): void {
      requireNode(nodeId).style[key] = value;
    },
    removeStyle(nodeId: NodeId, key: string): void {
      delete requireNode(nodeId).style[key];
    },
    addEventListener(
      nodeId: NodeId,
      event: string,
      callbackId: number,
      capture = false,
      passive = false,
    ): void {
      const callbacks = (requireNode(nodeId).listeners[event] ??= []);
      if (callbacks.includes(callbackId)) return;
      callbacks.push(callbackId);
      phases.set(callbackId, { capture, passive });
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
    focusNode(): void {},
    blurNode(_nodeId: number): void {},
    focusedNode(): NodeId | null {
      return null;
    },
    dispatchEvent(): boolean {
      return true;
    },
    click(): void {},
  };

  return nodes;
}

/**
 * Runs the listeners of `event` along `chain` (root first, target last): the
 * capture ones going down, then the bubble ones going up. Returns the phase
 * the host holds for each callback it ran, in order.
 */
function walk(chain: FakeNode[], event: string): { capture: boolean; passive: boolean }[] {
  const ran: { capture: boolean; passive: boolean }[] = [];
  const run = (node: FakeNode, capture: boolean): void => {
    for (const callbackId of node.listeners[event] ?? []) {
      const phase = phases.get(callbackId)!;
      if (phase.capture !== capture) continue;
      ran.push(phase);
      globalThis.__inca_callbacks__[callbackId]!();
    }
  };
  for (const node of chain) run(node, true);
  for (const node of chain.toReversed()) run(node, false);
  return ran;
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
    const rootId = createNode("root");
    root = {
      id: rootId,
      kind: "element",
      parent: null,
      children: [],
      focus: () => focus(rootId),
      blur: () => blur(rootId),
      click: () => click(rootId),
      ...createEventTarget(() => rootId),
      tabIndex: -1,
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

  it("registers .capture and .passive handlers with the host's phase and passivity", async () => {
    const App = {
      setup() {
        return () =>
          h("div", { onClickCapture: () => {} }, [
            h("div", {
              onClick: () => {},
              onClickCapturePassive: () => {},
            }),
          ]);
      },
    };

    createIncaApp(App).mount(root);
    await nextTick();

    const parent = nodes.get(nodes.get(root.id)!.children[0]!)!;
    const child = nodes.get(parent.children[0]!)!;
    expect(walk([parent, child], "click")).toEqual([
      { capture: true, passive: false }, // parent's onClickCapture
      { capture: true, passive: true }, // child's onClickCapturePassive
      { capture: false, passive: false }, // child's onClick
    ]);
  });

  it("lets a template ref add and remove listeners next to the template's own", async () => {
    const box = ref<IncaElement | null>(null);
    const log: string[] = [];
    const onRef = (): number => log.push("ref");
    const App = {
      setup() {
        return () => h("div", { ref: box, onClick: () => log.push("template") });
      },
    };

    createIncaApp(App).mount(root);
    await nextTick();
    const div = nodes.get(nodes.get(root.id)!.children[0]!)!;

    box.value!.addEventListener("click", onRef);
    box.value!.addEventListener("click", onRef);
    dispatch(div, "click");
    box.value!.removeEventListener("click", onRef);
    dispatch(div, "click");

    expect(log).toEqual(["template", "ref", "template"]);
    expect(div.listeners["click"]).toHaveLength(1);
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

  it("a ref of an element removed by v-if ignores listener and tabIndex calls", async () => {
    const state = reactive({ show: true });
    const button = ref<IncaElement | null>(null);
    const App = {
      setup() {
        return () => (state.show ? h("button", { ref: button }) : null);
      },
    };

    createIncaApp(App).mount(root);
    await nextTick();
    const handle = button.value!;

    const host = globalThis.__inca_native__;
    const spies = [
      vi.spyOn(host, "addEventListener"),
      vi.spyOn(host, "removeEventListener"),
      vi.spyOn(host, "dispatchEvent"),
      vi.spyOn(host, "getAttribute"),
      vi.spyOn(host, "setAttribute"),
    ];

    state.show = false;
    await nextTick();
    for (const spy of spies) spy.mockClear();

    const listener = (): void => {};
    let dispatched: boolean | undefined;
    handle.addEventListener("click", listener);
    handle.removeEventListener("click", listener);
    dispatched = handle.dispatchEvent(new Event("click"));
    handle.tabIndex = 5;

    expect(dispatched).toBe(true);
    expect(handle.tabIndex).toBe(0);
    for (const spy of spies) expect(spy.mock.calls).toEqual([]);
  });
});
