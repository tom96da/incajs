// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import { document } from "../document.mts";
import { HTML_NAMESPACE } from "../ffi/index.mts";
import { createElementNode, createIncaElement, rootElement } from "./handle.mts";
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
import type { ElementEntry, InterfacePrototypes, NodeEntry, Parsed } from "../ffi/index.mts";
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
    const el = createIncaElement(50, "div");
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
    const el = createIncaElement(1, ["Y", "urn:x", "x", "X:Y"]);

    markFreed([el.__node_id]);

    expect([el.tagName, el.localName, el.namespaceURI, el.prefix]).toEqual([
      "X:Y",
      "Y",
      "urn:x",
      "x",
    ]);
  });
});

describe("the prototype of a handle", () => {
  const SVG_NAMESPACE = "http://www.w3.org/2000/svg";
  const MATHML_NAMESPACE = "http://www.w3.org/1998/Math/MathML";
  const host = {
    createNode: vi.fn<(tag: string) => number>(() => 40),
    createElement: vi.fn<() => [number, string]>(() => [41, "div"]),
    createElementNS: vi.fn<() => ElementEntry>(),
    setAttribute: vi.fn<() => void>(),
    setStyle: vi.fn<() => void>(),
    holdNode: vi.fn<() => void>(),
    rootNodeId: vi.fn<() => number>(() => 2),
    templateContent: vi.fn<() => number>(() => 43),
    childNodeIds: vi.fn<() => number[]>(() => []),
    setInnerHtml: vi.fn<() => Parsed>(),
    cloneNode: vi.fn<() => NodeEntry[] | ElementEntry[]>(),
  };

  beforeEach(() => {
    vi.clearAllMocks();
    globalThis.__inca_native__ = nativeStub(host as never);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  /** Makes an element of the namespace with `createElementNS`. */
  const made = (namespace: string | null) => () => {
    host.createElementNS.mockReturnValueOnce([44, "x", namespace, null, "x"]);
    return document.createElementNS(namespace, "x");
  };

  /** Parses `entry` into a child of a new `div` and gives the handle of the child. */
  const parsed = (entry: Parsed[1][number]) => () => {
    host.setInnerHtml.mockReturnValueOnce([0, [entry]]);
    createElementNode("div").innerHTML = "x";
    return nodeOf(entry[0])!;
  };

  /** Copies a node of the host that answers `entries`, and gives the copy. */
  const copied = (entries: NodeEntry[] | ElementEntry[]) => () => {
    host.cloneNode.mockReturnValueOnce(entries);
    return createElementNode("div").cloneNode();
  };

  it.each([
    ["createElement", () => document.createElement("div"), "HTMLElement"],
    ["createElementNS with the HTML namespace", made(HTML_NAMESPACE), "HTMLElement"],
    ["createElementNS with the SVG namespace", made(SVG_NAMESPACE), "ForeignElement"],
    ["createElementNS with the MathML namespace", made(MATHML_NAMESPACE), "ForeignElement"],
    ["createElementNS with no namespace", made(null), "Element"],
    ["createElementNS with another namespace", made("urn:x"), "Element"],
    ["createTextNode", () => document.createTextNode("x"), "Text"],
    ["createComment", () => document.createComment("x"), "Comment"],
    ["createDocumentFragment", () => document.createDocumentFragment(), "DocumentFragment"],
    ["document", () => document, "HTMLDocument"],
    ["a copy of document", copied([[45, "#document", null]]), "HTMLDocument"],
    ["the mount container", () => rootElement(), "HTMLElement"],
    ["a parsed element", parsed([46, "b", HTML_NAMESPACE, 40, null]), "HTMLElement"],
    ["a parsed svg element", parsed([46, "b", SVG_NAMESPACE, 40, null]), "ForeignElement"],
    ["a parsed text node", parsed([47, "#text", null, 40, "t"]), "Text"],
    ["a parsed comment", parsed([48, "#comment", null, 40, "c"]), "Comment"],
    ["a cloned element", copied([[49, "div", HTML_NAMESPACE, null, "div"]]), "HTMLElement"],
    ["a cloned text node", copied([[50, "#text", "t"]]), "Text"],
    ["a cloned comment", copied([[51, "#comment", null]]), "Comment"],
    [
      "the content of a template",
      () => (createElementNode("template") as IncaTemplateElement).content,
      "DocumentFragment",
    ],
  ] as const)("is the prototype of %s", (_path, make, name) => {
    expect(Object.getPrototypeOf(make())).toBe(nativeStub().interfacePrototypes()[name]);
  });

  it("stays on the stored prototype when a script overwrites the globals", () => {
    const element = Object.getPrototypeOf(createElementNode("div"));
    vi.stubGlobal("Element", class {});
    vi.stubGlobal("HTMLElement", class {});

    expect(Object.getPrototypeOf(createElementNode("div"))).toBe(element);
  });

  it("asks the host for the prototypes once", async () => {
    const interfacePrototypes = vi.fn<() => InterfacePrototypes>(() =>
      nativeStub().interfacePrototypes(),
    );
    globalThis.__inca_native__ = nativeStub({ interfacePrototypes } as never);
    vi.resetModules();
    const lifecycle = await import("./lifecycle.mts");

    lifecycle.prototypes();
    lifecycle.prototypes();

    expect(interfacePrototypes).toHaveBeenCalledTimes(1);
  });
});
