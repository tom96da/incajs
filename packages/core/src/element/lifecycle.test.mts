// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import { createElementNode, createIncaElement } from "./handle.mts";
import {
  entered,
  isDestroyed,
  isRemoved,
  markFreed,
  markRemoved,
  nodeOf,
  registerNode,
  elementOf,
} from "./lifecycle.mts";
import type { IncaNode, IncaTemplateElement } from "./types.mts";

/** A stand-in handle with the given node id and kind. */
function standIn(id: number, kind: "element" | "text" = "element"): IncaNode {
  return Object.defineProperties(
    {},
    { __node_id: { value: id }, kind: { value: kind } },
  ) as IncaNode;
}

describe("the node table", () => {
  it("replaces the handle of an id when a newer handle registers under it", () => {
    const first = standIn(33);
    const el = standIn(33);
    registerNode(first);
    registerNode(el);

    expect(elementOf(33)).toBe(el);
  });

  it("replaces an attached handle when a newer handle registers under its id", () => {
    const first = standIn(34);
    const el = standIn(34);
    registerNode(first);
    entered(first);
    registerNode(el);

    expect(elementOf(34)).toBe(el);
  });

  it.each([
    ["attached", (handle: IncaNode) => entered(handle)],
    ["parentless", () => {}],
  ] as const)(
    "reads a freed %s handle as destroyed and gives no handle for its id",
    (_state, prepare) => {
      const text = standIn(41, "text");
      registerNode(text);
      prepare(text);

      markFreed([41]);

      expect([isDestroyed(text), nodeOf(41)]).toEqual([true, undefined]);
    },
  );

  it.each([
    ["attached", (handle: IncaNode) => entered(handle)],
    ["parentless", () => {}],
  ] as const)("reads a live %s handle as not destroyed", (_state, prepare) => {
    const text = standIn(43, "text");
    registerNode(text);
    prepare(text);

    expect(isDestroyed(text)).toBe(false);
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

describe("the release of a removed node", () => {
  it("registers a node that a held path covers once", async () => {
    const registered = new Map<object, number>();
    vi.stubGlobal(
      "FinalizationRegistry",
      class {
        register(target: object): void {
          registered.set(target, (registered.get(target) ?? 0) + 1);
        }
      },
    );
    vi.resetModules();
    globalThis.__inca_native__ = nativeStub({ releaseNode: () => {} } as never);
    const { createIncaElement } = await import("./handle.mts");
    const { leftHeld, releaseWhenCollected } = await import("./lifecycle.mts");
    const el = createIncaElement(50, "element", "div");
    releaseWhenCollected(el);

    leftHeld(el, [el.__node_id]);

    expect(registered.get(el)).toBe(1);
    vi.unstubAllGlobals();
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

    markFreed([owner.__node_id, content.__node_id]);

    expect(isDestroyed(content)).toBe(true);
  });

  it("marks its content handle removed when it leaves the tree", () => {
    const { owner, content } = template();

    markRemoved([owner.__node_id, content.__node_id]);

    expect(isRemoved(content)).toBe(true);
  });
});

describe("the name members of a freed element", () => {
  it("keep their values", () => {
    globalThis.__inca_native__ = nativeStub({ releaseNode: () => {} } as never);
    const el = createIncaElement(1, "element", ["Y", "urn:x", "x", "X:Y"]);

    markFreed([el.__node_id]);

    expect([el.tagName, el.localName, el.namespaceURI, el.prefix]).toEqual([
      "X:Y",
      "Y",
      "urn:x",
      "x",
    ]);
  });
});
