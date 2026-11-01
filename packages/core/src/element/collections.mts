// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { HTMLCollectionOf, IncaElement } from "./types.mts";

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

/**
 * Builds a live, read-only `HTMLCollection` over the elements `source` returns. Every read asks
 * `source` again.
 * @param source - lists the current elements in order
 * @returns the collection
 */
export function htmlCollectionOf<T extends IncaElement>(
  source: () => readonly T[],
): HTMLCollectionOf<T> {
  /** Throws unless `self` is the collection, like the brand check of a DOM method. */
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
      value(this: unknown, ...args: [index: unknown]) {
        brand(this);
        if (args.length < 1) throw new TypeError("item() needs 1 argument");
        return source()[(args[0] as number) >>> 0] ?? null;
      },
    },
    namedItem: {
      ...data,
      value(this: unknown, ...args: [name: unknown]) {
        brand(this);
        if (args.length < 1) throw new TypeError("namedItem() needs 1 argument");
        return namedElement(source(), `${args[0] as string}`);
      },
    },
    [Symbol.iterator]: {
      ...data,
      enumerable: false,
      value: Array.prototype.values,
    },
    [Symbol.toStringTag]: { value: "HTMLCollection", configurable: true },
  }) as object;
  const target = Object.create(members) as object;
  const named = (key: string): T | null => namedElement(source(), key);
  const own = (key: PropertyKey): boolean => Object.hasOwn(target, key);
  /** Whether a write to `key` is refused: an index, `length` or a name no expando hides. */
  const blocked = (key: string): boolean =>
    isIndex(key) || key === "length" || (!own(key) && !(key in members) && named(key) !== null);
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
        if (key === "length") return true;
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
      return [
        ...Array.from(items, (_, at) => String(at)),
        ...supportedNames(items).filter(
          (name) => !isIndex(name) && !(name in members) && !own(name),
        ),
        ...Reflect.ownKeys(target),
        "length",
      ];
    },
    getOwnPropertyDescriptor(_, key) {
      if (typeof key === "symbol" || own(key)) return Reflect.getOwnPropertyDescriptor(target, key);
      const items = source();
      if (key === "length") {
        return { value: items.length, writable: false, enumerable: false, configurable: true };
      }
      if (isIndex(key)) {
        const value = items[Number(key)];
        return value ? { value, writable: false, enumerable: true, configurable: true } : undefined;
      }
      const value = key in members ? undefined : namedElement(items, key);
      return value ? { value, writable: false, enumerable: false, configurable: true } : undefined;
    },
  }) as HTMLCollectionOf<T>;
  return proxy;
}
