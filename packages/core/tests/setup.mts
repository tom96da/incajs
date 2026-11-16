// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { vi } from "vitest";

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
  return {
    documentNodeId: () => 1,
    releaseNode: () => undefined,
    interfacePrototypes: () => Object.assign(Object.create(null), interfaceChain()),
    ...extra,
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
  globalThis.__inca_native__ = nativeStub(host as never);
  return host;
}
