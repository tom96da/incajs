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

import { callbackIdFor, groupOf, registeredGroupCount } from "../src/callbacks.mts";
import { createElementNode, createTextNode } from "../src/element/handle.mts";
import { elementOf, isDestroyed } from "../src/element/lifecycle.mts";
import { createIncaApp } from "../src/vue/index.mts";
import { nativeStub } from "./setup.mts";
import type { IncaElement } from "../src/index.mts";
import type { HostCallback } from "../src/native.mts";

/** Node's timers, put back after a test loads core over the fake host. */
const nodeTimers = { setTimeout, setInterval, clearTimeout, clearInterval };

/** The function the fake host keeps for each registration, as the callback table does. */
const hostCallbacks = new Map<number, HostCallback>();

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
let nextCallbackId: number;
const ROOT = 1;
/** Ids stay unique across tests, so a handle finalized late frees no node of a later test. */
let nextId = ROOT + 1;

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
  for (const callbackId of node.listeners) hostCallbacks.delete(callbackId);
  for (const child of node.children) freeSubtree(child, freed, callbacks);
}

function installNative(): void {
  nodes.clear();
  held.clear();
  orphans.clear();
  releaseNode.mockClear();
  nextCallbackId = 0;
  const create = (tag: string): number => {
    const id = nextId++;
    nodes.set(id, { tag, parent: null, children: [], listeners: [] });
    return id;
  };
  nodes.set(ROOT, { tag: "root", parent: null, children: [], listeners: [] });
  const insertBefore = (parentId: number, childId: number, refId: number | null): number => {
    const parent = nodeOf(parentId);
    nodeOf(childId);
    if (refId !== null && !parent.children.includes(refId)) {
      throw new DOMException("not a child", "NotFoundError");
    }
    detach(childId);
    orphans.delete(childId);
    const at = refId === null ? -1 : parent.children.indexOf(refId);
    if (at === -1) parent.children.push(childId);
    else parent.children.splice(at, 0, childId);
    nodeOf(childId).parent = parentId;
    return parent.children.indexOf(childId);
  };
  globalThis.__inca_native__ = nativeStub({
    rootNodeId: () => ROOT,
    createNode(tag) {
      const id = create(tag);
      orphans.add(id);
      return id;
    },
    appendChild: (parentId, childId) => insertBefore(parentId, childId, null),
    insertBefore,
    replaceChild: () => {
      throw new Error("not used here");
    },
    cloneNode: () => {
      throw new Error("not used here");
    },
    rootOf: () => {
      throw new Error("not used here");
    },
    compareNodePosition: () => {
      throw new Error("not used here");
    },
    isEqualNode: () => {
      throw new Error("not used here");
    },
    removeChild(parentId, childId) {
      nodeOf(parentId);
      if (!nodes.has(childId)) return;
      if (nodes.get(childId)?.parent !== parentId) {
        throw new DOMException("not a child", "NotFoundError");
      }
      detach(childId);
    },
    setAttribute: (id) => void nodeOf(id),
    getAttribute: (id) => (nodeOf(id), null),
    removeAttribute: (id) => void nodeOf(id),
    setStyle: (id) => void nodeOf(id),
    removeStyle: (id) => void nodeOf(id),
    inputValue: (id) => (nodeOf(id), ""),
    setInputValue: (id) => void nodeOf(id),
    addEventListener(id, _event, callback) {
      const callbackId = nextCallbackId++;
      nodeOf(id as number).listeners.push(callbackId);
      hostCallbacks.set(callbackId, callback);
      return callbackId;
    },
    removeEventListener(id, _event, callbackId) {
      const listeners = nodeOf(id as number).listeners;
      const at = listeners.indexOf(callbackId);
      if (at !== -1) listeners.splice(at, 1);
      hostCallbacks.delete(callbackId);
      return at !== -1;
    },
    attributeNames: (id) => (nodeOf(id), []),
    matches: (id) => (nodeOf(id), false),
    closest: (id) => (nodeOf(id), null),
    checkSelector: () => {},
    templateContent: () => null,
    innerHtml: () => "",
    outerHtml: () => "",
    setInnerHtml: () => [0, []],
    setOuterHtml: () => null,
    insertAdjacentHtml: () => [0, []],
    querySelector: (id) => (nodeOf(id), null),
    querySelectorAll: (id) => (nodeOf(id), []),
    releaseListeners(id) {
      const callbacks: number[] = [];
      const pending = [id];
      while (pending.length > 0) {
        const node = nodes.get(pending.pop()!);
        if (!node) continue;
        pending.push(...node.children);
        callbacks.push(...node.listeners);
        for (const callbackId of node.listeners) hostCallbacks.delete(callbackId);
        node.listeners = [];
      }
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
    setPointerCapture() {},
    releasePointerCapture() {},
    hasPointerCapture: () => false,
    linkedNode(id, link) {
      const node = nodeOf(id);
      const siblings = node.parent === null ? [] : nodeOf(node.parent).children;
      const at = siblings.indexOf(id);
      const found = {
        parent: node.parent,
        first: node.children[0],
        last: node.children.at(-1),
        previous: siblings[at - 1],
        next: siblings[at + 1],
      }[link];
      return found ?? null;
    },
    childNodeIds: (id) => [...nodeOf(id).children],
    isConnected(id) {
      let at: number | null = id;
      while (at !== null && at !== ROOT) at = nodeOf(at).parent;
      return at === ROOT;
    },
    textContent: () => "",
    wholeText: () => "",
    substringData: () => "",
    replaceData() {},
    splitText(id) {
      nodeOf(id);
      const tail = create("#text");
      orphans.add(tail);
      return tail;
    },
  });
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
  for (const callbackId of nodeOf(id).listeners) hostCallbacks.get(callbackId)!();
}

function idsOf(tag: string): number[] {
  return [...nodes].filter(([, node]) => node.tag === tag).map(([id]) => id);
}

const mountedApps: { unmount(): void }[] = [];

/** Mounts `App` on the fake host and unmounts it after the test, while its nodes still exist. */
function mountApp(App: Component): void {
  const app = createIncaApp(App);
  mountedApps.push(app);
  app.mount();
}

beforeEach(() => {
  installNative();
  hostCallbacks.clear();
  vi.spyOn(console, "warn").mockImplementation(() => {});
  vi.spyOn(console, "info").mockImplementation(() => {});
});

afterEach(() => {
  for (const app of mountedApps.splice(0)) app.unmount();
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

    mountApp(App);
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

  it("keeps a removed element in the host across frames while its handle lives", async () => {
    const show = ref(true);
    const button = ref<IncaElement | null>(null);
    let kept: IncaElement | null = null;
    const App = {
      render: () => h("div", [show.value ? h("button", { ref: button }) : null]),
    };
    mountApp(App);
    await settle();
    kept = button.value;
    const id = kept!.__node_id;

    show.value = false;
    await settle();
    await settle();

    expect(nodes.get(id)?.parent).toBeNull();
    expect(isDestroyed(kept!)).toBe(false);
  });

  it("keeps the Teleport anchors of a null target until the target is set", async () => {
    // The children are empty because Vue cannot patch the children of a Teleport mounted with a null target.
    const target = ref<IncaElement | null>(null);
    const box = ref<IncaElement | null>(null);
    const App = {
      render: () => h("div", [h("div", { ref: box }), h(Teleport, { to: target.value }, [])]),
    };

    mountApp(App);
    await settle();
    await settle();
    target.value = box.value;
    await settle();

    const tags = nodeOf(box.value!.__node_id).children.map((id) => nodes.get(id)!.tag);
    expect(tags).toEqual(["#text"]);
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

    mountApp(App);
    await settle();
    await settle();
    expect(idsOf("strong")).toHaveLength(1);
    finish();
    await settle();

    expect(idsOf("em")).toHaveLength(1);
    expect(idsOf("p").every((id) => nodes.get(id)!.parent === null)).toBe(true);
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

    const groups = registeredGroupCount();
    try {
      mountApp(App);
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
    expect(registeredGroupCount()).toBe(groups);
    expect(isDestroyed(span)).toBe(true);
    expect(nodes.size).toBe(1);
  });
});

describe("splitText on a text node with no parent", () => {
  it("keeps the new node through the next frame", () => {
    const tail = createTextNode("ab").splitText(1);

    frame();

    expect(isDestroyed(tail)).toBe(false);
  });
});

describe("__inca_released__", () => {
  it("releases the callbacks it is given", () => {
    const el = createElementNode("div");
    const listener = () => {};
    el.addEventListener("click", listener);
    const callbackId = nodeOf(el.__node_id).listeners[0]!;

    globalThis.__inca_released__([el.__node_id, 9999], [callbackId]);

    expect(callbackIdFor(groupOf(el.__node_id, "click", false), listener)).toBeUndefined();
  });

  it("marks the handles of the freed ids destroyed", () => {
    const el = createElementNode("div");
    const listener = () => {};
    el.addEventListener("click", listener);
    const callbackId = nodeOf(el.__node_id).listeners[0]!;

    globalThis.__inca_released__([el.__node_id, 9999], [callbackId]);

    expect(isDestroyed(el)).toBe(true);
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
    Object.assign(globalThis, nodeTimers);
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
