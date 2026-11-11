// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import { createElementNode, createIncaElement } from "./handle.mts";
import { isDestroyed, isRemoved, markDestroyed, markRemoved } from "./lifecycle.mts";
import type { IncaElement, IncaNode, IncaTemplateElement } from "./types.mts";

describe("element registry", () => {
  /** What `FinalizationRegistry.register` received, by target: one entry per registry. */
  const registered = new Map<object, { callback: (id: number) => void; held: number }[]>();
  /** Targets the test declares collected. */
  const gone = new WeakSet<object>();

  /** Declares `target` collected: its WeakRef reads empty and its registry callbacks run. */
  function collect(target: object): void {
    gone.add(target);
    for (const { callback, held } of registered.get(target)!) callback(held);
  }

  /** A stand-in handle with the given node id. */
  function handleOf(id: number): IncaElement {
    return Object.defineProperties(
      {},
      {
        __node_id: { value: id },
        kind: { value: "element" },
      },
    ) as IncaElement;
  }

  async function load() {
    vi.resetModules();
    return import("./lifecycle.mts");
  }

  beforeEach(() => {
    registered.clear();
    vi.stubGlobal(
      "FinalizationRegistry",
      class {
        constructor(private readonly callback: (id: number) => void) {}
        register(target: object, held: number): void {
          const list = registered.get(target) ?? [];
          list.push({ callback: this.callback, held });
          registered.set(target, list);
        }
      },
    );
    vi.stubGlobal(
      "WeakRef",
      class {
        constructor(private readonly target: object) {}
        deref(): object | undefined {
          return gone.has(this.target) ? undefined : this.target;
        }
      },
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("drops the record of a collected handle", async () => {
    const { recordedCount, registerNode } = await load();
    const before = recordedCount();
    const el = handleOf(31);
    registerNode(el);
    expect(recordedCount()).toBe(before + 1);

    collect(el);

    expect(recordedCount()).toBe(before);
  });

  it("keeps the record of a newer handle when an older one with its id is collected", async () => {
    const { elementOf, registerNode } = await load();
    const first = handleOf(33);
    const el = handleOf(33);
    registerNode(first);
    registerNode(el);

    collect(first);

    expect(elementOf(33)).toBe(el);
  });

  /** A stand-in text handle with the given node id. */
  function textOf(id: number): IncaNode {
    return Object.defineProperties(
      {},
      {
        __node_id: { value: id },
        kind: { value: "text" },
      },
    ) as IncaNode;
  }

  it("marks the handle of a freed text node destroyed", async () => {
    const { isDestroyed, markFreed, registerNode } = await load();
    const text = textOf(41);
    registerNode(text);

    markFreed([41]);

    expect(isDestroyed(text)).toBe(true);
  });

  it("drops the record of a destroyed text node", async () => {
    const { markDestroyed, nodeOf, registerNode } = await load();
    const text = textOf(42);
    registerNode(text);

    markDestroyed(text);

    expect(nodeOf(42)).toBeUndefined();
  });
});

describe("the release of a template", () => {
  const registered = new Map<object, { callback: (id: number) => void; held: number }[]>();
  const host = {
    createNode: vi.fn<(tag: string) => number>(() => 4),
    templateContent: vi.fn<(nodeId: number) => number | null>(() => 5),
    holdNode: vi.fn<(nodeId: number) => void>(),
    releaseNode: vi.fn<(nodeId: number) => void>(),
  };

  beforeEach(() => {
    registered.clear();
    vi.clearAllMocks();
    vi.stubGlobal(
      "FinalizationRegistry",
      class {
        constructor(private readonly callback: (id: number) => void) {}
        register(target: object, held: number): void {
          registered.set(target, [
            ...(registered.get(target) ?? []),
            { callback: this.callback, held },
          ]);
        }
      },
    );
    globalThis.__inca_native__ = nativeStub(host as never);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("releases the node of a collected template handle once, with its id", async () => {
    vi.resetModules();
    const { createElementNode } = await import("./handle.mts");
    const owner = createElementNode("template");

    for (const { callback, held } of registered.get(owner)!) callback(held);

    expect(host.releaseNode.mock.calls).toEqual([[owner.__node_id]]);
  });
});

describe("a template", () => {
  const host = {
    createNode: vi.fn<(tag: string) => number>(() => 4),
    templateContent: vi.fn<(nodeId: number) => number | null>(() => 5),
    holdNode: vi.fn<(nodeId: number) => void>(),
    releaseNode: vi.fn<(nodeId: number) => void>(),
  };

  const template = () => {
    globalThis.__inca_native__ = nativeStub(host as never);
    const owner = createElementNode("template") as IncaTemplateElement;
    return { owner, content: owner.content };
  };

  it("marks its content handle destroyed when the host frees it", () => {
    const { owner, content } = template();

    markDestroyed(owner);

    expect(isDestroyed(content)).toBe(true);
  });

  it("marks its content handle removed when it leaves the tree", () => {
    const { owner, content } = template();

    markRemoved(owner);

    expect(isRemoved(content)).toBe(true);
  });
});

describe("the name members of a freed element", () => {
  it("keep their values", () => {
    globalThis.__inca_native__ = nativeStub();
    const el = createIncaElement(1, "element", ["Y", "urn:x", "x", "X:Y"]);

    markDestroyed(el);

    expect([el.tagName, el.localName, el.namespaceURI, el.prefix]).toEqual([
      "X:Y",
      "Y",
      "urn:x",
      "x",
    ]);
  });
});
