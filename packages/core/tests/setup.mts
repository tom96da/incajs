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

/** The data of the text nodes and comments by node id, made once per realm. */
const DATA_KEY = Symbol.for("incajs.test.data");

function dataStore(): Map<number, string> {
  const shared = globalThis as { [DATA_KEY]?: Map<number, string> };
  return (shared[DATA_KEY] ??= new Map());
}

beforeEach(() => dataStore().clear());

/**
 * Sets the data of a text node or comment that a test builds without a host.
 * @param id - the node id
 * @param data - the data
 */
export function declareData(id: number, data: string): void {
  dataStore().set(id, data);
}

/** The text of a value, as a host member converts an argument. */
function asText(value: unknown): string {
  // oxlint-disable-next-line typescript/no-base-to-string
  return String(value ?? "");
}

/** The native object a stand-in member reads through. */
function natives(): Record<string, ((...args: never[]) => unknown) | undefined> {
  return globalThis.__inca_native__ as never;
}

/** Defines the getter `key` of `proto`, enumerable and configurable. */
function defineGetter(proto: object, key: string, get: (this: { __node_id: number }) => unknown) {
  Object.defineProperty(proto, key, { get, enumerable: true, configurable: true });
}

/** Defines the accessor `key` of `proto`. */
function defineAccessor(
  proto: object,
  key: string,
  get: (this: { __node_id: number }) => unknown,
  set: (this: { __node_id: number }, value: never) => void,
) {
  Object.defineProperty(proto, key, { get, set, enumerable: true, configurable: true });
}

/** Defines the method `key` of `proto`. */
function defineMethod(proto: object, key: string, run: (id: number, args: unknown[]) => unknown) {
  Object.defineProperty(proto, key, {
    value(this: { __node_id: number }, ...args: unknown[]) {
      return run(this.__node_id, args);
    },
    writable: true,
    enumerable: true,
    configurable: true,
  });
}

/**
 * The stand-in for the members the host defines on the node interfaces. A test of one of these
 * members runs in Rust over the real host. These keep the members that other tests use to read a
 * node.
 * @param chain - the stand-in prototypes
 */
function defineHostMembers(chain: InterfaceChain): void {
  const members = { localName: 0, namespaceURI: 1, prefix: 2, tagName: 3 } as const;
  for (const [key, index] of Object.entries(members)) {
    defineGetter(chain.Element, key, function () {
      return partsOf(this.__node_id)[index];
    });
  }
  const read = (id: number, name: string): string | null => {
    const value = natives().getAttribute?.(id as never, name as never) as unknown;
    return value === null || value === undefined ? null : asText(value);
  };
  const data = (id: number): string => asText(natives().textContent?.(id as never));
  defineGetter(chain.Node, "nodeType", function () {
    const declared = declaredOf(this.__node_id);
    return { element: 1, text: 3, comment: 8, fragment: 11, document: 9 }[declared.kind];
  });
  defineGetter(chain.Node, "nodeName", function () {
    const declared = declaredOf(this.__node_id);
    if (declared.kind === "element") return partsOf(this.__node_id)[3];
    return declared.kind === "fragment" ? "#document-fragment" : `#${declared.kind}`;
  });
  defineAccessor(
    chain.Node,
    "nodeValue",
    function () {
      const kind = declaredOf(this.__node_id).kind;
      return kind === "text" || kind === "comment" ? data(this.__node_id) : null;
    },
    function (value: unknown) {
      const kind = declaredOf(this.__node_id).kind;
      if (kind !== "text" && kind !== "comment") return;
      natives().setCharacterData?.(this.__node_id as never, asText(value) as never);
    },
  );
  defineGetter(chain.Node, "textContent", function () {
    const kind = declaredOf(this.__node_id).kind;
    if (kind === "document") return null;
    return data(this.__node_id);
  });
  defineGetter(chain.Node, "isConnected", function () {
    const n = natives();
    return (n.rootOf?.(this.__node_id as never) as unknown) === (n.documentNodeId?.() as unknown);
  });
  defineMethod(chain.Node, "hasChildNodes", (id) => {
    const children = natives().childNodeIds?.(id as never) as unknown[] | undefined;
    return (children?.length ?? 0) > 0;
  });
  defineAccessor(
    chain.CharacterData,
    "data",
    function () {
      return data(this.__node_id);
    },
    function (value: unknown) {
      const text = value === null ? "" : asText(value);
      natives().setCharacterData?.(this.__node_id as never, text as never);
    },
  );
  defineGetter(chain.CharacterData, "length", function () {
    return data(this.__node_id).length;
  });
  defineGetter(chain.Text, "wholeText", function () {
    return data(this.__node_id);
  });
  defineAccessor(
    chain.Element,
    "id",
    function () {
      return read(this.__node_id, "id") ?? "";
    },
    function (value: unknown) {
      natives().setAttribute?.(this.__node_id as never, "id" as never, asText(value) as never);
    },
  );
  defineAccessor(
    chain.Element,
    "className",
    function () {
      return read(this.__node_id, "class") ?? "";
    },
    function (value: unknown) {
      natives().setAttribute?.(this.__node_id as never, "class" as never, asText(value) as never);
    },
  );
  defineMethod(chain.Element, "getAttribute", (id, [name]) => read(id, String(name)));
  defineMethod(chain.Element, "hasAttribute", (id, [name]) => read(id, String(name)) !== null);
  defineMethod(chain.Element, "setAttribute", (id, [name, value]) =>
    natives().setAttribute?.(id as never, String(name) as never, asText(value) as never),
  );
  defineMethod(chain.Element, "removeAttribute", (id, [name]) =>
    natives().removeAttribute?.(id as never, String(name) as never),
  );
  defineMethod(
    chain.Element,
    "getAttributeNames",
    (id) => natives().attributeNames?.(id as never) ?? [],
  );
  defineMethod(chain.Element, "matches", () => false);
  defineGetter(chain.Element, "innerHTML", () => "");
  defineGetter(chain.Element, "outerHTML", () => "");
  for (const proto of [chain.Element, chain.DocumentFragment, chain.Document]) {
    defineGetter(proto, "childElementCount", function () {
      return (natives().childCount?.(this.__node_id as never, true as never) as number) ?? 0;
    });
  }
  defineMethod(chain.HTMLElement, "click", (id) => natives().click?.(id as never));
  for (const proto of [chain.HTMLElement, chain.ForeignElement]) {
    defineMethod(proto, "focus", (id, [options]) =>
      natives().focusNode?.(
        id as never,
        Boolean((options as { preventScroll?: boolean } | undefined)?.preventScroll) as never,
      ),
    );
    defineMethod(proto, "blur", (id) => natives().blurNode?.(id as never));
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
  defineHostMembers(chain as unknown as InterfaceChain);
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
    textContent: (id) => dataStore().get(id) ?? "",
    setCharacterData: (id, data) => void dataStore().set(id, data),
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
