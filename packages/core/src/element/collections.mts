// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type {
  Attr,
  HTMLCollectionOf,
  IncaElement,
  IncaNode,
  NamedNodeMap,
  NodeListOf,
} from "./types.mts";

const ARRAY_INDEX = /^(?:0|[1-9]\d*)$/;

/** The largest array index plus one: the keys from here on are named properties. */
const INDEX_LIMIT = 2 ** 32 - 1;

/** Whether `key` is an array index such as `"0"` or `"12"`. */
function isIndex(key: string): boolean {
  return ARRAY_INDEX.test(key) && Number(key) < INDEX_LIMIT;
}

/** The first element in `elements` with an `id` of `name`, else with a `name` attribute of it. */
function namedElement<T extends IncaElement>(elements: readonly T[], name: string): T | null {
  if (name === "") return null;
  return (
    elements.find((el) => el.id === name) ??
    elements.find((el) => el.getAttribute("name") === name) ??
    null
  );
}

/** The `id` and `name` values of `elements` in tree order, each once and never empty. */
function supportedNames(elements: readonly IncaElement[]): string[] {
  const names = new Set<string>();
  for (const el of elements) {
    names.add(el.id);
    names.add(el.getAttribute("name") ?? "");
  }
  names.delete("");
  return [...names];
}

/** What distinguishes one kind of collection from another. */
interface Kind<T extends object> {
  /** The `Symbol.toStringTag` value. */
  tag: string;
  /** The name of the lookup method, `namedItem` or `getNamedItem`; a list has none. */
  method?: string;
  /** Whether the object also has forEach, entries, keys and values. */
  arrayMembers?: boolean;
  /** Lists the current items in order. */
  source: () => readonly T[];
  /** The result of the lookup method for `name`. */
  find: (name: string) => T | null;
  /** The item a named property `key` reads, or `null`. */
  named: (key: string) => T | null;
  /** The supported property names of `items`. */
  names: (items: readonly T[]) => string[];
}

/** Builds a read-only indexed object. A kind with no named items has no named properties. */
function indexedCollection<T extends object>(kind: Kind<T>): object {
  const { source, named } = kind;
  /** Throws unless `self` is the collection. */
  const brand = (self: unknown): void => {
    if (self !== proxy) throw new TypeError("the receiver is not a collection. call it on one");
  };
  const data = { writable: true, enumerable: true, configurable: true };
  const members = Object.create(Object.prototype, {
    length: {
      get(this: unknown) {
        brand(this);
        return source().length;
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
          return source()[(args[0] as number) >>> 0] ?? null;
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
          return kind.find(`${args[0] as string}`);
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
      if (isIndex(key)) return source()[Number(key)];
      if (key in target) return Reflect.get(target, key, receiver);
      return named(key) ?? undefined;
    },
    has(_, key) {
      if (typeof key === "symbol" || key in target) return key in target;
      return isIndex(key) ? Number(key) < source().length : named(key) !== null;
    },
    set(_, key, value, receiver) {
      if (receiver === proxy && typeof key === "string" && blocked(key)) return false;
      return Reflect.set(target, key, value, receiver);
    },
    deleteProperty(_, key) {
      if (typeof key === "string") {
        if (isIndex(key)) return Number(key) >= source().length;
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
      const items = source();
      const indices = Array.from(items, (_, at) => String(at));
      const names = kind
        .names(items)
        .filter((name) => !isIndex(name) && !(name in members) && !own(name));
      const expandos = Reflect.ownKeys(target);
      return [...indices, ...expandos, ...names];
    },
    getOwnPropertyDescriptor(_, key) {
      if (typeof key === "symbol" || own(key)) return Reflect.getOwnPropertyDescriptor(target, key);
      if (isIndex(key)) {
        const value = source()[Number(key)];
        return value ? { value, writable: false, enumerable: true, configurable: true } : undefined;
      }
      const value = key in members ? undefined : named(key);
      return value ? { value, writable: false, enumerable: false, configurable: true } : undefined;
    },
  });
  return proxy;
}

/**
 * Builds a live, read-only `HTMLCollection` over the elements `source` returns. Every read asks
 * `source` again.
 * @param source - lists the current elements in order
 * @returns the collection
 */
export function htmlCollectionOf<T extends IncaElement>(
  source: () => readonly T[],
): HTMLCollectionOf<T> {
  const find = (name: string): T | null => namedElement(source(), name);
  return indexedCollection({
    tag: "HTMLCollection",
    method: "namedItem",
    source,
    find,
    named: find,
    names: supportedNames,
  }) as HTMLCollectionOf<T>;
}

/**
 * Builds a live, read-only `NamedNodeMap` over the attributes `source` returns. Every read asks
 * `source` again.
 * @param source - lists the current attributes in order
 * @param find - answers `getNamedItem()` for a name
 * @param named - answers a named property: the attribute whose name is `key`, or `null`
 * @returns the map
 */
export function namedNodeMapOf(
  source: () => readonly Attr[],
  find: (name: string) => Attr | null,
  named: (key: string) => Attr | null,
): NamedNodeMap {
  return indexedCollection({
    tag: "NamedNodeMap",
    method: "getNamedItem",
    source,
    find,
    named,
    names: (items) => items.map((attr) => attr.name),
  }) as NamedNodeMap;
}

/**
 * Builds a live, read-only `NodeList` over the nodes `source` returns. Every read asks `source`
 * again.
 * @param source - lists the current nodes in order
 * @returns the list
 */
export function nodeListOf<T extends IncaNode>(source: () => readonly T[]): NodeListOf<T> {
  return indexedCollection({
    tag: "NodeList",
    arrayMembers: true,
    source,
    find: () => null,
    named: () => null,
    names: () => [],
  }) as NodeListOf<T>;
}
