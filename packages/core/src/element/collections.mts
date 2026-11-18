// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { Node } from "./nodeTypes.mts";
import type { Attr, HTMLCollectionOf, Element, NamedNodeMap, NodeListOf } from "./types.mts";

const ARRAY_INDEX = /^(?:0|[1-9]\d*)$/;

/** The largest array index plus one: the keys from here on are named properties. */
const INDEX_LIMIT = 2 ** 32 - 1;

/** Whether `key` is an array index such as `"0"` or `"12"`. */
function isIndex(key: string): boolean {
  return ARRAY_INDEX.test(key) && Number(key) < INDEX_LIMIT;
}

/** The reads a collection makes of its items. */
export interface ItemList<T> {
  /** The number of items. */
  length: () => number;
  /** The item at `index`, `null` or `undefined` past the last one. */
  at: (index: number) => T | null | undefined;
}

/** The reads a collection with named items makes. */
export interface ItemSource<T> extends ItemList<T> {
  /** The item the lookup method and a named property find for `name`, or `null`. */
  named: (name: string) => T | null;
  /** The supported property names in order. */
  names: () => string[];
}

/** What distinguishes one kind of collection from another. */
interface Kind<T extends object> {
  /** The `Symbol.toStringTag` value. */
  tag: string;
  /** The name of the lookup method, `namedItem` or `getNamedItem`; a list has none. */
  method?: string;
  /** Whether the object also has forEach, entries, keys and values. */
  arrayMembers?: boolean;
  /** Reads the items. A kind with no named items has no named properties. */
  items: ItemSource<T>;
}

/** Builds a read-only indexed object. */
function indexedCollection<T extends object>(kind: Kind<T>): object {
  const { items } = kind;
  const { named } = items;
  /** Throws unless `self` is the collection. */
  const brand = (self: unknown): void => {
    if (self !== proxy) throw new TypeError("the receiver is not a collection. call it on one");
  };
  const data = { writable: true, enumerable: true, configurable: true };
  const members = Object.create(Object.prototype, {
    length: {
      get(this: unknown) {
        brand(this);
        return items.length();
      },
      enumerable: true,
      configurable: true,
    },
    item: {
      ...data,
      value: Object.defineProperty(
        function (this: unknown, ...args: [index: unknown]) {
          brand(this);
          if (args.length < 1) throw new TypeError("item() needs 1 argument");
          return items.at((args[0] as number) >>> 0) ?? null;
        },
        "length",
        { value: 1 },
      ),
    },
    ...(kind.method && {
      [kind.method]: {
        ...data,
        value(this: unknown, ...args: [name: unknown]) {
          brand(this);
          if (args.length < 1) throw new TypeError(`${kind.method}() needs 1 argument`);
          return named(`${args[0] as string}`);
        },
      },
    }),
    ...(kind.arrayMembers && {
      forEach: { ...data, value: Array.prototype.forEach },
      entries: { ...data, value: Array.prototype.entries },
      keys: { ...data, value: Array.prototype.keys },
      values: { ...data, value: Array.prototype.values },
    }),
    [Symbol.iterator]: {
      ...data,
      enumerable: false,
      value: Array.prototype.values,
    },
    [Symbol.toStringTag]: { value: kind.tag, configurable: true },
  }) as object;
  const target = Object.create(members) as object;
  const own = (key: PropertyKey): boolean => Object.hasOwn(target, key);
  /** Whether a write to `key` is refused: an index or a name no expando hides. */
  const blocked = (key: string): boolean =>
    isIndex(key) || (!own(key) && !(key in members) && named(key) !== null);
  const proxy = new Proxy(target, {
    get(_, key, receiver) {
      if (typeof key === "symbol") return Reflect.get(target, key, receiver);
      if (isIndex(key)) return items.at(Number(key)) ?? undefined;
      if (key in target) return Reflect.get(target, key, receiver);
      return named(key) ?? undefined;
    },
    has(_, key) {
      if (typeof key === "symbol" || key in target) return key in target;
      return isIndex(key) ? Number(key) < items.length() : named(key) !== null;
    },
    set(_, key, value, receiver) {
      if (receiver === proxy && typeof key === "string" && blocked(key)) return false;
      return Reflect.set(target, key, value, receiver);
    },
    deleteProperty(_, key) {
      if (typeof key === "string") {
        if (isIndex(key)) return Number(key) >= items.length();
        if (blocked(key)) return false;
      }
      return Reflect.deleteProperty(target, key);
    },
    // A non-extensible target would invalidate the descriptors the traps report.
    preventExtensions: () => false,
    defineProperty(_, key, descriptor) {
      if (typeof key === "string" && blocked(key)) return false;
      return Reflect.defineProperty(target, key, descriptor);
    },
    ownKeys() {
      const indices = Array.from({ length: items.length() }, (_, at) => String(at));
      const names = items
        .names()
        .filter((name) => !isIndex(name) && !(name in members) && !own(name));
      const expandos = Reflect.ownKeys(target);
      return [...indices, ...expandos, ...names];
    },
    getOwnPropertyDescriptor(_, key) {
      if (typeof key === "symbol" || own(key)) return Reflect.getOwnPropertyDescriptor(target, key);
      if (isIndex(key)) {
        const value = items.at(Number(key));
        return value ? { value, writable: false, enumerable: true, configurable: true } : undefined;
      }
      const value = key in members ? undefined : named(key);
      return value ? { value, writable: false, enumerable: false, configurable: true } : undefined;
    },
  });
  return proxy;
}

/**
 * Builds a live, read-only `HTMLCollection`. Every read asks `items` again.
 * @param items - reads the current elements
 * @returns the collection
 */
export function htmlCollectionOver<T extends Element>(items: ItemSource<T>): HTMLCollectionOf<T> {
  return indexedCollection({
    tag: "HTMLCollection",
    method: "namedItem",
    items,
  }) as HTMLCollectionOf<T>;
}

/**
 * Builds a live, read-only `NamedNodeMap` over the attributes `source` returns. Every read asks
 * `source` again.
 * @param source - lists the current attributes in order
 * @param find - answers `getNamedItem()` and a named property: the attribute called `name`, or
 *   `null`
 * @returns the map
 */
export function namedNodeMapOf(
  source: () => readonly Attr[],
  find: (name: string) => Attr | null,
): NamedNodeMap {
  return indexedCollection({
    tag: "NamedNodeMap",
    method: "getNamedItem",
    items: {
      length: () => source().length,
      at: (index) => source()[index],
      named: find,
      names: () => source().map((attr) => attr.name),
    },
  }) as NamedNodeMap;
}

/**
 * Builds a live, read-only `NodeList`. Every read asks `items` again.
 * @param items - reads the current nodes
 * @returns the list
 */
export function nodeListOver<T extends Node>(items: ItemList<T>): NodeListOf<T> {
  return indexedCollection({
    tag: "NodeList",
    arrayMembers: true,
    items: { ...items, named: () => null, names: () => [] },
  }) as NodeListOf<T>;
}

/**
 * Builds a live, read-only `NodeList` over the nodes `source` returns. Every read asks `source`
 * again.
 * @param source - lists the current nodes in order
 * @returns the list
 */
export function nodeListOf<T extends Node>(source: () => readonly T[]): NodeListOf<T> {
  return nodeListOver({ length: () => source().length, at: (index) => source()[index] });
}
