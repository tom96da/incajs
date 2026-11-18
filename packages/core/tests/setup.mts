// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, vi } from "vitest";

import type { ElementName } from "../src/ffi/index.mts";

// The globals the host defines before an app runs.

// A test file imports this module for `nativeStub`, so the QuickJS run evaluates it a second time
// after its own stubs. The chain of node interfaces is made once per realm, under a shared key,
// so a second evaluation gives the same objects.

/** The prototypes of the node interfaces, each with the name of its parent. */
const NODE_INTERFACES = [
  ["EventTarget", null],
  ["Node", "EventTarget"],
  ["CharacterData", "Node"],
  ["Text", "CharacterData"],
  ["Comment", "CharacterData"],
  ["Element", "Node"],
  ["HTMLElement", "Element"],
  ["ForeignElement", "Element"],
  ["DocumentFragment", "Node"],
  ["Document", "Node"],
  ["HTMLDocument", "Document"],
] as const;

type InterfaceChain = Record<(typeof NODE_INTERFACES)[number][0], object>;

const CHAIN_KEY = Symbol.for("incajs.test.interfaceChain");

const HTML = "http://www.w3.org/1999/xhtml";
const SVG = "http://www.w3.org/2000/svg";
const MATHML = "http://www.w3.org/1998/Math/MathML";

/** What a test declares about a node id, which the stand-in `nodeHandle` reads. */
export type Declared =
  | { kind: "element"; name: ElementName }
  | { kind: "text" | "comment" | "fragment" | "document" };

const DECLARED_KEY = Symbol.for("incajs.test.declared");

/** The declarations by node id, made once per realm so a second evaluation shares them. */
function declarations(): Map<number, Declared> {
  const shared = globalThis as { [DECLARED_KEY]?: Map<number, Declared> };
  return (shared[DECLARED_KEY] ??= new Map());
}

beforeEach(() => declarations().clear());

/** The declaration of an id with none: the document for its id, an HTML `div` for any other. */
function declaredOf(id: number): Declared {
  const documentId = globalThis.__inca_native__?.documentNodeId?.();
  return (
    declarations().get(id) ??
    (id === documentId ? { kind: "document" } : { kind: "element", name: "div" })
  );
}

/**
 * Declares the kind and name of a node id. An id with no declaration is an HTML `div`.
 * @param id - the node id
 * @param declared - the kind, and the name of an element
 */
export function declareNode(id: number, declared: Declared): void {
  declarations().set(id, declared);
}

/** The declaration of each tag that names a kind other than an element. */
export const KINDS: Record<string, Declared> = {
  "#text": { kind: "text" },
  "#comment": { kind: "comment" },
  "#fragment": { kind: "fragment" },
  "#document": { kind: "document" },
};

/** The name parts of the element `id`: local name, namespace, prefix and tag name. */
function partsOf(id: number): [string, string | null, string | null, string] {
  const declared = declaredOf(id);
  const name = declared.kind === "element" ? declared.name : "div";
  return typeof name === "string" ? [name, HTML, null, name.toUpperCase()] : name;
}

/**
 * The stand-in for the name members the host defines on `Element`.
 * @param proto - the stand-in prototype of `Element`
 */
function defineNameMembers(proto: object): void {
  const members = { localName: 0, namespaceURI: 1, prefix: 2, tagName: 3 } as const;
  for (const [key, index] of Object.entries(members)) {
    Object.defineProperty(proto, key, {
      get(this: { __node_id: number }) {
        return partsOf(this.__node_id)[index];
      },
      enumerable: true,
      configurable: true,
    });
  }
}

/**
 * The stand-in for the native `nodeHandle`: an object of the stand-in prototype that the
 * declaration of `id` names, with the own property `__node_id`.
 * @param id - the node id
 * @param parented - whether the node has a parent
 * @returns the result of the native
 */
export function standInNodeHandle(
  id: number,
  parented = false,
): Native["nodeHandle"] extends (...args: never[]) => infer R ? R : never {
  const declared = declaredOf(id);
  const chain = interfaceChain();
  let proto: object;
  let special: "input" | "template" | null = null;
  if (declared.kind === "element") {
    const [local, space] = partsOf(id);
    proto =
      space === HTML
        ? chain.HTMLElement
        : space === SVG || space === MATHML
          ? chain.ForeignElement
          : chain.Element;
    if (space === HTML && (local === "input" || local === "template")) special = local;
  } else {
    proto = {
      text: chain.Text,
      comment: chain.Comment,
      fragment: chain.DocumentFragment,
      document: chain.HTMLDocument,
    }[declared.kind];
  }
  const handle = Object.create(proto) as object;
  Object.defineProperty(handle, "__node_id", { value: id });
  return [handle, parented, special];
}

/**
 * The stand-in for the prototypes of the host: plain objects in the inheritance of the node
 * interfaces. Each has the string tag of its name, the foreign element prototype excepted.
 */
function interfaceChain(): InterfaceChain {
  const shared = globalThis as { [CHAIN_KEY]?: InterfaceChain };
  if (shared[CHAIN_KEY]) return shared[CHAIN_KEY];
  const chain: Record<string, object> = {};
  for (const [name, parent] of NODE_INTERFACES) {
    const prototype = Object.create(parent === null ? Object.prototype : chain[parent]!);
    if (name !== "ForeignElement") {
      Object.defineProperty(prototype, Symbol.toStringTag, { value: name, configurable: true });
    }
    chain[name] = prototype;
  }
  defineNameMembers(chain.Element!);
  return (shared[CHAIN_KEY] = chain as InterfaceChain);
}

for (const [name] of NODE_INTERFACES) {
  if (name === "ForeignElement" || name in globalThis) continue;
  const interfaceObject = function () {};
  Object.defineProperty(interfaceObject, "name", { value: name });
  interfaceObject.prototype = interfaceChain()[name];
  Object.defineProperty(globalThis, name, {
    value: interfaceObject,
    writable: true,
    configurable: true,
  });
}

/** A minimal `DOMRect` for the tests, defined when the global is missing. */
class DOMRectStandIn {
  constructor(
    public x = 0,
    public y = 0,
    public width = 0,
    public height = 0,
  ) {}
  get top() {
    return Math.min(this.y, this.y + this.height);
  }
  get right() {
    return Math.max(this.x, this.x + this.width);
  }
  get bottom() {
    return Math.max(this.y, this.y + this.height);
  }
  get left() {
    return Math.min(this.x, this.x + this.width);
  }
  toJSON() {
    const { x, y, width, height, top, right, bottom, left } = this;
    return { x, y, width, height, top, right, bottom, left };
  }
}

if (!("DOMRect" in globalThis)) {
  Object.defineProperty(globalThis, "DOMRect", {
    value: DOMRectStandIn,
    writable: true,
    configurable: true,
  });
}

const SIGNAL_KEY = Symbol.for("incajs.test.abortSignal");

/** The stand-in `AbortSignal` and the ids of its instances. */
interface SignalKit {
  AbortSignal: new () => object;
  ids: WeakMap<object, number>;
}

/**
 * The stand-in for the `AbortSignal` of the host: a class over the `EventTarget` stand-in whose
 * instances have ids, made once per realm. An object made with `Object.create` has none.
 */
function signalKit(): SignalKit {
  const shared = globalThis as { [SIGNAL_KEY]?: SignalKit };
  if (shared[SIGNAL_KEY]) return shared[SIGNAL_KEY];
  const ids = new WeakMap<object, number>();
  let issued = 0;
  const AbortSignal = function (this: object) {
    ids.set(this, ++issued);
  } as unknown as new () => object;
  AbortSignal.prototype = Object.create(interfaceChain().EventTarget);
  Object.defineProperty(AbortSignal.prototype, Symbol.toStringTag, {
    value: "AbortSignal",
    configurable: true,
  });
  return (shared[SIGNAL_KEY] = { AbortSignal, ids });
}

Object.defineProperty(globalThis, "AbortSignal", {
  value: signalKit().AbortSignal,
  writable: true,
  configurable: true,
});

/** The host object core calls. */
type Native = typeof globalThis.__inca_native__;

/**
 * A stand-in for `__inca_native__` with the calls core makes while it builds `document`, the
 * prototypes of the node interfaces, and the release that the collection of a handle makes at any
 * time.
 * @param extra - the calls and values a test adds or replaces
 * @returns the stand-in, in which `documentNodeId()` gives 1
 */
export function nativeStub(extra: Partial<Native> = {}): Native {
  const { createNode, createElement, createElementNS } = extra;
  // A node a test creates is declared from the tag or the name its creating native gets or gives.
  const declaring: Partial<Native> = {};
  if (createNode) {
    declaring.createNode = (tag) => {
      const id = createNode(tag);
      declareNode(id, KINDS[tag] ?? { kind: "element", name: tag });
      return id;
    };
  }
  if (createElement) {
    declaring.createElement = (name) => {
      const entry = createElement(name);
      declareNode(entry[0], { kind: "element", name: entry[1] });
      return entry;
    };
  }
  if (createElementNS) {
    declaring.createElementNS = (namespace, name) => {
      const entry = createElementNS(namespace, name);
      declareNode(entry[0], { kind: "element", name: [entry[1], entry[2], entry[3], entry[4]] });
      return entry;
    };
  }
  return {
    documentNodeId: () => 1,
    releaseNode: () => undefined,
    holdNode: () => undefined,
    rootNodeId: () => -1,
    textContent: () => "",
    registerUncloneable: () => undefined,
    windowTargetId: () => "window-target",
    signalId: (value) => signalKit().ids.get(value as object) ?? null,
    interfacePrototypes: () => Object.assign(Object.create(null), interfaceChain()),
    nodeHandle: (id) => standInNodeHandle(id),
    ...extra,
    ...declaring,
  } as Native;
}

globalThis.__inca_native__ ??= nativeStub();

/** `[x, y, width, height, border, scrollX, scrollY, gutterWidth, gutterHeight]` of a drawn node. */
type Box = readonly [number, number, number, number, number, number, number, number, number];

/**
 * Installs a fake host whose reads give `boxes`, `scroll` and the parents in `parents`, and returns
 * it.
 */
export function installMembersHost(
  boxes: Record<number, Box> = {},
  scroll: readonly [number, number, number, number] | null = null,
  parents: Record<number, number> = {},
) {
  const host = {
    linkedNode: vi.fn<(nodeId: number, link: string) => number | null>((nodeId, link) =>
      link === "parent" ? (parents[nodeId] ?? null) : null,
    ),
    getAttribute: vi.fn<() => null>(() => null),
    layoutOf: vi.fn<(nodeId: number) => Box | null>((nodeId) => boxes[nodeId] ?? null),
    scrollOf: vi.fn<() => readonly [number, number, number, number] | null>(() => scroll),
    scrollNode: vi.fn<(nodeId: number, left: number | null, top: number | null) => void>(),
  };
  vi.stubGlobal("__inca_native__", nativeStub(host as never));
  return host;
}
