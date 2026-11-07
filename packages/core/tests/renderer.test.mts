// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { h, nextTick, reactive, ref } from "@vue/runtime-core";
import { beforeEach, describe, expect, it } from "vitest";
import type { Ref } from "@vue/runtime-core";

import { document } from "../src/document.mts";
import { htmlCollectionOf, namedNodeMapOf, nodeListOf } from "../src/element/collections.mts";
import { rectOf } from "../src/element/geometry.mts";
import { createIncaElement } from "../src/element/handle.mts";
import { blur, click, focus } from "../src/events.mts";
import { createEventTarget } from "../src/eventTarget.mts";
import { createNode, rootNodeId } from "../src/tree.mts";
import { createIncaApp } from "../src/vue/index.mts";
import { vModelText, withDirectives } from "../src/vue/runtime.mts";
import type { NodeMutationMembers } from "../src/element/nodeMutation.mts";
import type { NodeRelationMembers } from "../src/element/nodeRelations.mts";
import type { EventHandlers } from "../src/element/types.mts";
import type { IncaElement, NodeId } from "../src/index.mts";
import type { HostCallback } from "../src/native.mts";

/** The handler properties, absent: nothing here reads them. */
const noHandlers = {} as EventHandlers;

/** The mutation members, absent: nothing here calls them. */
const noMutation = {} as Pick<IncaElement, keyof NodeMutationMembers | keyof NodeRelationMembers>;

/**
 * What the fake host knows of each registration beyond its node and event.
 */
const phases = new Map<number, { capture: boolean; passive: boolean }>();

/** The function the fake host keeps for each registration, as the callback table does. */
const hostCallbacks = new Map<number, HostCallback>();

/** The text of each input node in the fake host. */
const inputValues = new Map<NodeId, string>();

/** The node the fake host reports as focused. */
let focused: NodeId | null = null;

interface FakeNode {
  tag: string;
  attributes: Record<string, unknown>;
  style: Record<string, unknown>;
  parent: NodeId | null;
  children: NodeId[];
  listeners: Record<string, number[]>;
}

// A minimal, in-memory stand-in for the native host's retained tree, real
// enough to drive `incajs`'s actual wrapper functions and `incajs/vue`'s
// actual `nodeOps`/`patchProp` end to end, without a real Rust process.
//
// It matches the real host where the difference is observable: one parent per
// node, several callbacks per (node, event), and `releaseListeners` dropping a whole
// subtree. It does not refuse a cycle the way the host does. `nodeOps` never
// builds one.
function installFakeNative(): Map<NodeId, FakeNode> {
  const nodes = new Map<NodeId, FakeNode>();
  let nextId = 0;
  let nextCallbackId = 0;

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
    appendChild(parentId: NodeId, childId: NodeId): number {
      return globalThis.__inca_native__.insertBefore(parentId, childId, null);
    },
    insertBefore(parentId: NodeId, childId: NodeId, refId: NodeId | null): number {
      const parent = requireNode(parentId);
      const child = requireNode(childId);
      const anchorIndex = refId === null ? -1 : parent.children.indexOf(refId);
      if (refId !== null && anchorIndex === -1) {
        throw new DOMException("not a child", "NotFoundError");
      }
      detach(childId);
      const at = refId === null ? -1 : parent.children.indexOf(refId);
      if (at === -1) {
        parent.children.push(childId);
      } else {
        parent.children.splice(at, 0, childId);
      }
      child.parent = parentId;
      return parent.children.indexOf(childId);
    },
    replaceChild(): number {
      throw new Error("not used here");
    },
    cloneNode(): [NodeId, string][] {
      throw new Error("not used here");
    },
    rootOf(id: NodeId): NodeId {
      let top = id;
      while (requireNode(top).parent !== null) top = requireNode(top).parent!;
      return top;
    },
    compareNodePosition(): number {
      throw new Error("not used here");
    },
    isEqualNode(): boolean {
      throw new Error("not used here");
    },
    removeChild(parentId: NodeId, childId: NodeId): void {
      requireNode(parentId);
      if (!nodes.has(childId)) return;
      if (nodes.get(childId)?.parent !== parentId) {
        throw new DOMException("not a child", "NotFoundError");
      }
      detach(childId);
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
    linkedNode(nodeId: NodeId, link: "parent" | "first" | "last" | "previous" | "next") {
      const node = requireNode(nodeId);
      if (link === "parent") return node.parent;
      if (link === "first") return node.children[0] ?? null;
      if (link === "last") return node.children.at(-1) ?? null;
      const siblings = node.parent === null ? [] : requireNode(node.parent).children;
      return siblings[siblings.indexOf(nodeId) + (link === "next" ? 1 : -1)] ?? null;
    },
    childNodeIds: (nodeId: NodeId) => [...requireNode(nodeId).children],
    isConnected(nodeId: NodeId): boolean {
      let at: NodeId | null = nodeId;
      while (at !== null && at !== rootId) at = requireNode(at).parent;
      return at === rootId;
    },
    textContent: () => "",
    wholeText: () => "",
    substringData: () => "",
    replaceData() {},
    splitText: () => 0,
    matches: () => false,
    closest: () => null,
    checkSelector: () => {},
    querySelector: () => null,
    querySelectorAll: () => [],
    attributeNames(nodeId: NodeId): string[] {
      return Object.keys(requireNode(nodeId).attributes).sort();
    },
    setStyle(nodeId: NodeId, key: string, value: unknown): void {
      requireNode(nodeId).style[key] = value;
    },
    removeStyle(nodeId: NodeId, key: string): void {
      delete requireNode(nodeId).style[key];
    },
    inputValue(nodeId: NodeId): string {
      requireNode(nodeId);
      return inputValues.get(nodeId) ?? "";
    },
    setInputValue(nodeId: NodeId, value: string): void {
      requireNode(nodeId);
      inputValues.set(nodeId, value);
    },
    addEventListener(
      nodeId: NodeId,
      event: string,
      callback: HostCallback,
      capture = false,
      passive = false,
    ): number {
      const callbacks = (requireNode(nodeId).listeners[event] ??= []);
      const callbackId = nextCallbackId++;
      callbacks.push(callbackId);
      phases.set(callbackId, { capture, passive });
      hostCallbacks.set(callbackId, callback);
      return callbackId;
    },
    removeEventListener(nodeId: NodeId, event: string, callbackId: number): boolean {
      const callbacks = nodes.get(nodeId)?.listeners[event];
      if (!callbacks) return false;
      const index = callbacks.indexOf(callbackId);
      if (index === -1) return false;
      callbacks.splice(index, 1);
      hostCallbacks.delete(callbackId);
      return true;
    },
    releaseListeners(nodeId: NodeId): number[] {
      const released: number[] = [];
      const pending = [nodeId];
      while (pending.length > 0) {
        const node = nodes.get(pending.pop()!);
        if (!node) continue;
        pending.push(...node.children);
        released.push(...Object.values(node.listeners).flat());
        node.listeners = {};
      }
      for (const callbackId of released) hostCallbacks.delete(callbackId);
      return released;
    },
    holdNode(): void {},
    releaseNode(): void {},
    focusNode(): void {},
    blurNode(_nodeId: number): void {},
    focusedNode(): NodeId | null {
      return focused;
    },
    dispatchEvent(): boolean {
      return true;
    },
    click(): void {},
    setPointerCapture(): void {},
    releasePointerCapture(): void {},
    hasPointerCapture: () => false,
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
      hostCallbacks.get(callbackId)!();
    }
  };
  for (const node of chain) run(node, true);
  for (const node of chain.toReversed()) run(node, false);
  return ran;
}

function dispatch(node: FakeNode, event: string, payload?: unknown): void {
  const callbackIds = node.listeners[event] ?? [];
  if (callbackIds.length === 0) throw new Error(`no ${event} listener registered`);
  for (const callbackId of callbackIds) {
    const callback = hostCallbacks.get(callbackId);
    if (callback === undefined) throw new Error(`no callback registered as ${callbackId}`);
    callback(payload);
  }
}

describe("incajs/vue renderer, driven end to end through real core internals", () => {
  let nodes: Map<NodeId, FakeNode>;
  let root: IncaElement;

  beforeEach(() => {
    nodes = installFakeNative();
    inputValues.clear();
    focused = null;
    hostCallbacks.clear();
    const rootId = createNode("root");
    root = {
      ...noHandlers,
      ...noMutation,
      getAttribute: () => null,
      setAttribute: () => {},
      hasAttribute: () => false,
      removeAttribute: () => {},
      getAttributeNames: () => [],
      attributes: namedNodeMapOf(
        () => [],
        () => null,
        () => null,
      ),
      __node_id: rootId,
      id: "",
      kind: "element",
      tagName: "DIV",
      matches: () => false,
      closest: () => null,
      querySelector: () => null,
      querySelectorAll: () => nodeListOf(() => []),
      parentElement: null,
      nodeType: 1,
      nodeName: "DIV",
      nodeValue: null,
      textContent: "",
      isConnected: false,
      ownerDocument: document,
      parentNode: null,
      firstChild: null,
      lastChild: null,
      previousSibling: null,
      nextSibling: null,
      childNodes: nodeListOf(() => []),
      hasChildNodes: () => false,
      firstElementChild: null,
      lastElementChild: null,
      childElementCount: 0,
      previousElementSibling: null,
      nextElementSibling: null,
      parent: null,
      __children: [],
      children: htmlCollectionOf(() => []),
      focus: () => focus(rootId),
      blur: () => blur(rootId),
      contains: () => false,
      click: () => click(rootId),
      setPointerCapture: () => {},
      releasePointerCapture: () => {},
      hasPointerCapture: () => false,
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
      getRootNode: () => root,
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

    const rootNode = nodes.get(root.__node_id)!;
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

    const containerId = nodes.get(root.__node_id)!.children[0]!;
    const originalOrder = nodes.get(containerId)!.children.slice();
    expect(originalOrder).toHaveLength(3);

    state.items = [3, 1, 2];
    await nextTick();

    const reordered = nodes.get(containerId)!.children;
    expect(reordered).toEqual([originalOrder[2], originalOrder[0], originalOrder[1]]);
  });

  it("releases every callback inside a removed subtree and keeps its nodes", async () => {
    const state = reactive({ shown: true });
    const App = {
      setup() {
        return () =>
          h("div", null, [state.shown ? h("div", null, [h("div", { onClick: () => {} })]) : null]);
      },
    };

    createIncaApp(App).mount(root);
    await nextTick();

    const containerId = nodes.get(root.__node_id)!.children[0]!;
    const branchId = nodes.get(containerId)!.children[0]!;
    const leafId = nodes.get(branchId)!.children[0]!;
    expect(hostCallbacks.size).toBe(1);

    state.shown = false;
    await nextTick();

    expect(nodes.get(branchId)?.parent).toBeNull();
    expect(
      nodes.has(leafId),
      "Vue removes only the subtree's root, so the descendant leaves with it",
    ).toBe(true);
    expect(hostCallbacks.size).toBe(0);
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

    const parent = nodes.get(nodes.get(root.__node_id)!.children[0]!)!;
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
    const div = nodes.get(nodes.get(root.__node_id)!.children[0]!)!;

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

    const div = nodes.get(nodes.get(root.__node_id)!.children[0]!)!;
    state.armed = false;
    await nextTick();

    expect(div.listeners["click"]).toHaveLength(0);
    expect(hostCallbacks.size).toBe(0);
    expect(clicks).toBe(0);
  });
});

describe("v-model on an input", () => {
  let nodes: Map<NodeId, FakeNode>;
  let mountRoot: IncaElement;

  beforeEach(() => {
    nodes = installFakeNative();
    inputValues.clear();
    focused = null;
    hostCallbacks.clear();
    mountRoot = createIncaElement(rootNodeId(), "element");
  });

  /** Mounts an `input` bound to `model` with `modifiers`, and returns its node id and handle. */
  async function mountInput(model: Ref<string>, modifiers: Record<string, boolean> = {}) {
    const box = ref<IncaElement | null>(null);
    const App = {
      setup() {
        return () =>
          withDirectives(
            h("input", {
              ref: box,
              "onUpdate:modelValue": (value: string) => (model.value = value),
            }),
            [[vModelText, model.value, "", modifiers]],
          );
      },
    };
    createIncaApp(App).mount(mountRoot);
    await nextTick();
    return { id: nodes.get(rootNodeId())!.children[0]!, element: box.value! };
  }

  it("writes the model into the input on mount", async () => {
    const { id } = await mountInput(ref("hello"));

    expect(inputValues.get(id)).toBe("hello");
  });

  it("assigns the input's value to the model on input", async () => {
    const model = ref("");
    const { id, element } = await mountInput(model);
    inputValues.set(id, "typed");

    dispatch(nodes.get(id)!, "input", { target: element });

    expect(model.value).toBe("typed");
  });

  it("keeps the focused input's text when the trimmed model matches it", async () => {
    const model = ref("a ");
    const { id } = await mountInput(model, { trim: true });
    focused = id;

    model.value = "a";
    await nextTick();

    expect(inputValues.get(id)).toBe("a ");
  });
});
