// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import {
  KeepAlive,
  Suspense,
  Teleport,
  defineComponent,
  h,
  nextTick,
  ref,
} from "@vue/runtime-core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Component } from "@vue/runtime-core";

import { elementOf, isDestroyed } from "../src/element/lifecycle.mts";
import { createElementNode } from "../src/element/shadowTree.mts";
import { createIncaApp } from "../src/vue/index.mts";
import type { IncaElement } from "../src/index.mts";

interface FakeNode {
  tag: string;
  parent: number | null;
  children: number[];
  listeners: number[];
}

/**
 * A fake host with the orphan model of the real one: a created node is an
 * orphan until it is inserted or held, `removeChild` makes an orphan unless
 * the node is held, and `frame()` frees every orphan with its subtree and
 * reports the freed ids to `__inca_released__`.
 */
const nodes = new Map<number, FakeNode>();
const held = new Set<number>();
const orphans = new Set<number>();
const releaseNode = vi.fn<(id: number) => void>();
let nextId: number;
let nextCallbackId: number;
const ROOT = 1;

function nodeOf(id: number): FakeNode {
  const node = nodes.get(id);
  if (!node) throw new TypeError(`unknown node id: ${id}`);
  return node;
}

function detach(id: number): void {
  const node = nodeOf(id);
  if (node.parent === null) return;
  const siblings = nodeOf(node.parent).children;
  siblings.splice(siblings.indexOf(id), 1);
  node.parent = null;
  if (!held.has(id)) orphans.add(id);
}

function freeSubtree(id: number, freed: number[], callbacks: number[]): void {
  const node = nodes.get(id);
  if (!node) return;
  nodes.delete(id);
  held.delete(id);
  orphans.delete(id);
  freed.push(id);
  callbacks.push(...node.listeners);
  for (const child of node.children) freeSubtree(child, freed, callbacks);
}

function installNative(): void {
  nodes.clear();
  held.clear();
  orphans.clear();
  releaseNode.mockClear();
  nextId = ROOT;
  nextCallbackId = 0;
  const create = (tag: string): number => {
    const id = nextId++;
    nodes.set(id, { tag, parent: null, children: [], listeners: [] });
    return id;
  };
  create("root");
  const insertBefore = (parentId: number, childId: number, anchorId: number | null): void => {
    const parent = nodeOf(parentId);
    nodeOf(childId);
    detach(childId);
    orphans.delete(childId);
    const at = anchorId === null ? -1 : parent.children.indexOf(anchorId);
    if (at === -1) parent.children.push(childId);
    else parent.children.splice(at, 0, childId);
    nodeOf(childId).parent = parentId;
  };
  globalThis.__inca_native__ = {
    rootNodeId: () => ROOT,
    createNode(tag) {
      const id = create(tag);
      orphans.add(id);
      return id;
    },
    appendChild: (parentId, childId) => insertBefore(parentId, childId, null),
    insertBefore,
    removeChild(parentId, childId) {
      nodeOf(parentId);
      if (nodes.get(childId)?.parent === parentId) detach(childId);
    },
    setAttribute: (id) => void nodeOf(id),
    getAttribute: (id) => (nodeOf(id), null),
    removeAttribute: (id) => void nodeOf(id),
    setStyle: (id) => void nodeOf(id),
    removeStyle: (id) => void nodeOf(id),
    addEventListener(id) {
      const callbackId = nextCallbackId++;
      nodeOf(id as number).listeners.push(callbackId);
      return callbackId;
    },
    removeEventListener(id, _event, callbackId) {
      const listeners = nodeOf(id as number).listeners;
      const at = listeners.indexOf(callbackId);
      if (at !== -1) listeners.splice(at, 1);
      return at !== -1;
    },
    destroyNode(id) {
      if (!nodes.has(id)) return [];
      detach(id);
      const freed: number[] = [];
      const callbacks: number[] = [];
      freeSubtree(id, freed, callbacks);
      return callbacks;
    },
    holdNode(id) {
      nodeOf(id);
      held.add(id);
      orphans.delete(id);
    },
    releaseNode(id) {
      releaseNode(id);
      held.delete(id);
      const node = nodes.get(id);
      if (node && node.parent === null && id !== ROOT) orphans.add(id);
    },
    focusNode() {},
    blurNode() {},
    focusedNode: () => null,
    layoutOf: () => null,
    scrollOf: () => null,
    scrollNode() {},
    scrollIntoViewNode() {},
    dispatchEvent: () => true,
    click() {},
  };
}

/** Frees the orphans like the host does at the start of a frame. */
function frame(): void {
  const freed: number[] = [];
  const callbacks: number[] = [];
  for (const id of Array.from(orphans)) freeSubtree(id, freed, callbacks);
  if (freed.length > 0) globalThis.__inca_released__(freed, callbacks);
}

async function settle(): Promise<void> {
  await nextTick();
  frame();
  await nextTick();
  frame();
}

function click(id: number): void {
  for (const callbackId of nodeOf(id).listeners) globalThis.__inca_callbacks__[callbackId]!();
}

function idsOf(tag: string): number[] {
  return [...nodes].filter(([, node]) => node.tag === tag).map(([id]) => id);
}

beforeEach(() => {
  installNative();
  delete (globalThis as { __inca_callbacks__?: unknown }).__inca_callbacks__;
  vi.spyOn(console, "warn").mockImplementation(() => {});
  vi.spyOn(console, "info").mockImplementation(() => {});
});

afterEach(() => {
  vi.restoreAllMocks();
});

/**
 * A failure of the KeepAlive, Teleport or Suspense test means an update of Vue
 * changed how those components reach the node creators, so the held creators
 * in `createIncaApp` no longer apply to them.
 */
describe("a host that frees orphans, driven by the Inca renderer", () => {
  it("keeps a KeepAlive component's nodes and listeners across frames and toggles", async () => {
    const A = defineComponent({
      setup() {
        const count = ref(0);
        return () => h("button", { onClick: () => count.value++ }, `a${count.value}`);
      },
    });
    const B = { render: () => h("section", "b") };
    const which = ref<Component>(A);
    const App = { render: () => h(KeepAlive, null, [h(which.value)]) };

    createIncaApp(App).mount();
    await settle();
    const buttonId = idsOf("button")[0]!;
    click(buttonId);
    await settle();

    which.value = B;
    await settle();
    which.value = A;
    await settle();

    expect(idsOf("button")).toEqual([buttonId]);
    click(buttonId);
    await settle();
    expect(nodes.get(buttonId)!.listeners).toHaveLength(1);
    expect(elementOf(buttonId)).toBeDefined();
    expect(isDestroyed(elementOf(buttonId)!)).toBe(false);

    const before = nodes.size;
    for (let i = 0; i < 20; i++) {
      which.value = i % 2 === 0 ? B : A;
      await settle();
    }
    expect(nodes.size).toBe(before);
    expect([...held].some((id) => nodes.get(id)!.parent === null)).toBe(true);
  });

  it("keeps the Teleport anchors of a null target until the target is set", async () => {
    // The children are empty because Vue cannot patch the children of a Teleport mounted with a null target.
    const target = ref<IncaElement | null>(null);
    const box = ref<IncaElement | null>(null);
    const App = {
      render: () => h("div", [h("div", { ref: box }), h(Teleport, { to: target.value }, [])]),
    };

    createIncaApp(App).mount();
    await settle();
    await settle();
    target.value = box.value;
    await settle();

    const tags = nodeOf(box.value!.__node_id).children.map((id) => nodes.get(id)!.tag);
    expect(tags).toEqual(["text"]);
  });

  it("keeps the resolved branch of a pending Suspense across frames", async () => {
    let finish!: () => void;
    const Pending = defineComponent({
      async setup() {
        await new Promise<void>((resolve) => (finish = resolve));
        return () => h("em");
      },
    });
    const Ready = defineComponent({
      async setup() {
        await Promise.resolve();
        return () => h("strong");
      },
    });
    const App = {
      render: () =>
        h(Suspense, null, {
          default: () => h("div", [h(Ready), h(Pending)]),
          fallback: () => h("p"),
        }),
    };

    createIncaApp(App).mount();
    await settle();
    await settle();
    expect(idsOf("strong")).toHaveLength(1);
    finish();
    await settle();

    expect(idsOf("em")).toHaveLength(1);
    expect(idsOf("p")).toHaveLength(0);
    const strong = idsOf("strong")[0]!;
    let at: number | null = strong;
    while (at !== null && at !== ROOT) at = nodes.get(at)!.parent;
    expect(at).toBe(ROOT);
  });

  it("frees the nodes a throwing child setup leaves, with their callbacks and handles", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    const Bad = {
      setup() {
        throw new Error("setup failed");
      },
    };
    const App = { render: () => h("div", [h("span", { onClick: () => {} }), h(Bad)]) };

    try {
      createIncaApp(App).mount();
    } catch {
      // The failure is the case under test.
    }
    await nextTick();
    const spanId = idsOf("span")[0]!;
    const span = elementOf(spanId)!;
    const callbackIds = [...nodeOf(spanId).listeners];
    expect(callbackIds).toHaveLength(1);
    expect(orphans.size).toBeGreaterThan(0);

    frame();

    expect(nodes.has(spanId)).toBe(false);
    expect(globalThis.__inca_callbacks__[callbackIds[0]!]).toBeUndefined();
    expect(isDestroyed(span)).toBe(true);
    expect(nodes.size).toBe(1);
  });
});

describe("__inca_released__", () => {
  it("releases the callbacks it is given", () => {
    const el = createElementNode("div");
    const listener = () => {};
    el.addEventListener("click", listener);
    const callbackId = nodeOf(el.__node_id).listeners[0]!;

    globalThis.__inca_released__([el.__node_id, 9999], [callbackId]);

    expect(globalThis.__inca_callbacks__[callbackId]).toBeUndefined();
  });

  it("marks the handles of the freed ids destroyed", () => {
    const el = createElementNode("div");
    const listener = () => {};
    el.addEventListener("click", listener);
    const callbackId = nodeOf(el.__node_id).listeners[0]!;

    globalThis.__inca_released__([el.__node_id, 9999], [callbackId]);

    expect(isDestroyed(el)).toBe(true);
  });

  it("releases the rest when one release throws", () => {
    const a = createElementNode("div");
    const b = createElementNode("div");
    a.addEventListener("click", () => {});
    b.addEventListener("click", () => {});
    const first = nodeOf(a.__node_id).listeners[0]!;
    const second = nodeOf(b.__node_id).listeners[0]!;
    Object.defineProperty(globalThis.__inca_callbacks__, first, { configurable: false });

    expect(() => globalThis.__inca_released__([a.__node_id, b.__node_id], [first, second])).toThrow(
      TypeError,
    );

    expect(globalThis.__inca_callbacks__[second]).toBeUndefined();
    expect(isDestroyed(b)).toBe(true);
    expect(isDestroyed(a)).toBe(true);
  });

  it("stays in place when a script assigns to it", () => {
    const hook = globalThis.__inca_released__;

    expect(() => {
      globalThis.__inca_released__ = () => {};
    }).toThrow(TypeError);
    expect(globalThis.__inca_released__).toBe(hook);
  });

  it("is the latest copy's once core loads again", async () => {
    const hook = globalThis.__inca_released__;
    vi.resetModules();

    await import("../src/callbacks.mts");

    expect(globalThis.__inca_released__).not.toBe(hook);
  });
});

describe("the hold registry", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.resetModules();
  });

  it("registers the handle of a held node by id and releases the id on cleanup", async () => {
    class FakeRegistry {
      static instances: FakeRegistry[] = [];
      register = vi.fn<(target: object, held: number) => void>();
      constructor(readonly cleanup: (held: number) => void) {
        FakeRegistry.instances.push(this);
      }
    }
    vi.resetModules();
    vi.stubGlobal("FinalizationRegistry", FakeRegistry);
    const fresh = await import("../src/rendererCore.mts");
    const { createHeldNodeOps, createNodeOps } = await import("../src/vue/nodeOps.mts");
    const ops = createHeldNodeOps(fresh, createNodeOps(fresh));

    const handle = ops.createElement("div");

    const registering = FakeRegistry.instances.filter((r) =>
      r.register.mock.calls.some(
        ([target, held]) => target === handle && held === handle.__node_id,
      ),
    );
    expect(typeof handle.__node_id).toBe("number");
    expect(registering.length).toBeGreaterThan(0);
    for (const registry of registering) registry.cleanup(handle.__node_id);
    expect(releaseNode).toHaveBeenCalledWith(handle.__node_id);
    expect(held.has(handle.__node_id)).toBe(false);
  });
});
