// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { adoptElement } from "../../tests/adopt.mts";
import { declareNode, nativeStub, standInNodeHandle } from "../../tests/setup.mts";
import { adoptHandle, createElementNode, handleOf } from "./handle.mts";
import {
  CHARACTER_DATA_RULE,
  DOCUMENT_RULE,
  ELEMENT_RULE,
  HTML_ELEMENT_RULE,
  HTML_OR_SVG_RULE,
  PARENT_NODE_RULE,
  TEXT_RULE,
  entered,
  receiverOf,
  stateOf,
  isDestroyed,
  isRemoved,
  markFreed,
  markRemoved,
  nodeOf,
  registerNode,
  elementOf,
} from "./lifecycle.mts";
import type { Declared } from "../../tests/setup.mts";
import type { InterfacePrototypes } from "../ffi/index.mts";
import type { Receiver } from "./lifecycle.mts";
import type { Node } from "./nodeTypes.mts";
import type { Element, HTMLTemplateElement } from "./types.mts";

/** A stand-in handle with the given node id and kind. */
function standIn(id: number, kind: "element" | "text" = "element"): Node {
  return Object.defineProperties({}, { __node_id: { value: id }, kind: { value: kind } }) as Node;
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
    ["attached", (handle: Node) => entered(handle)],
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
    ["attached", (handle: Node) => entered(handle)],
    ["parentless", () => {}],
  ] as const)("reads a live %s handle as not destroyed", (_state, prepare) => {
    const text = standIn(43, "text");
    registerNode(text);
    prepare(text);

    expect(isDestroyed(text)).toBe(false);
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
    vi.stubGlobal("__inca_native__", nativeStub(host as never));
    const owner = createElementNode("template") as HTMLTemplateElement;
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

describe("the record of a handle built from the host", () => {
  const SVG_NAMESPACE = "http://www.w3.org/2000/svg";

  it.each<[string, Declared, string, Receiver]>([
    ["an HTML element", { kind: "element", name: "div" }, "element", HTML_ELEMENT_RULE],
    [
      "an SVG element",
      { kind: "element", name: ["g", SVG_NAMESPACE, null, "g"] },
      "element",
      HTML_OR_SVG_RULE,
    ],
    [
      "an element of no namespace",
      { kind: "element", name: ["a", null, null, "a"] },
      "element",
      ELEMENT_RULE,
    ],
    ["a text node", { kind: "text" }, "text", TEXT_RULE],
    ["a comment", { kind: "comment" }, "comment", CHARACTER_DATA_RULE],
    ["a fragment", { kind: "fragment" }, "fragment", PARENT_NODE_RULE],
    ["a document", { kind: "document" }, "document", DOCUMENT_RULE],
  ])("takes the kind and the rule of %s from its prototype", (_name, declared, kind, rule) => {
    declareNode(70, declared);

    const handle = adoptHandle(70);

    expect([stateOf(handle)!.kind, receiverOf(handle, rule)]).toEqual([kind, stateOf(handle)]);
  });

  it.each<[string, Declared, Receiver]>([
    [
      "an SVG element",
      { kind: "element", name: ["g", SVG_NAMESPACE, null, "g"] },
      HTML_ELEMENT_RULE,
    ],
    [
      "an element of no namespace",
      { kind: "element", name: ["a", null, null, "a"] },
      HTML_OR_SVG_RULE,
    ],
    ["an HTML element", { kind: "element", name: "div" }, TEXT_RULE],
  ])("refuses %s for a rule of another namespace or kind", (_name, declared, rule) => {
    declareNode(70, declared);

    expect(() => receiverOf(adoptHandle(70), rule)).toThrow(TypeError);
  });

  it("asks the host for the prototypes once", async () => {
    const interfacePrototypes = vi.fn<() => InterfacePrototypes>(() =>
      nativeStub().interfacePrototypes(),
    );
    vi.stubGlobal("__inca_native__", nativeStub({ interfacePrototypes } as never));
    vi.resetModules();
    const lifecycle = await import("./lifecycle.mts");

    lifecycle.prototypes();
    lifecycle.prototypes();

    expect(interfacePrototypes).toHaveBeenCalledTimes(1);
  });
});

describe("a handle built on demand", () => {
  const PARENT = 197;
  const CHILD = 198;
  const holdNode = vi.fn<(nodeId: number) => void>();

  /** The ways to reach the handle of the node `CHILD` from the handle of `PARENT`. */
  const PATHS: [string, (parent: Element) => unknown][] = [
    ["handleOf", () => handleOf(CHILD)],
    ["firstChild", (parent) => parent.firstChild],
    ["querySelector", (parent) => parent.querySelector("p")],
    ["closest", (parent) => parent.closest("p")],
  ];

  /** Installs a host that reports `CHILD` as parented or not, and returns the handle of `PARENT`. */
  function installHost(parented: boolean): Element {
    holdNode.mockClear();
    vi.stubGlobal(
      "__inca_native__",
      nativeStub({
        holdNode,
        nodeHandle: (id: number) => standInNodeHandle(id, parented),
        linkedNode: () => CHILD,
        querySelector: () => CHILD,
        closest: () => CHILD,
      } as never),
    );
    declareNode(CHILD, { kind: "element", name: "div" });
    const parent = adoptElement(PARENT, "div");
    entered(parent);
    return parent;
  }

  beforeEach(() => {
    vi.stubGlobal(
      "WeakRef",
      class {
        deref(): undefined {
          return undefined;
        }
      },
    );
  });

  afterEach(() => {
    markFreed([PARENT, CHILD]);
  });

  it.each(PATHS)("is strong when reached by %s and its node has a parent", (_name, reach) => {
    const parent = installHost(true);

    reach(parent);

    expect(nodeOf(CHILD)).toBeDefined();
  });

  it.each(PATHS)(
    "is weak and held when reached by %s and its node has no parent",
    (_name, reach) => {
      const parent = installHost(false);

      reach(parent);

      expect(nodeOf(CHILD)).toBeUndefined();
      expect(holdNode).toHaveBeenCalledWith(CHILD);
    },
  );
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
    vi.stubGlobal("__inca_native__", nativeStub(host as never));
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
    vi.stubGlobal("__inca_native__", nativeStub({ releaseNode: () => {} } as never));
    const { leftHeld, releaseWhenCollected } = await import("./lifecycle.mts");
    const el = adoptElement(50, "div");
    releaseWhenCollected(el);

    leftHeld(el, [el.__node_id]);

    expect(registered.get(el)).toBe(1);
  });
});
