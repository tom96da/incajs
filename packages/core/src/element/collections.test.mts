// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it } from "vitest";

import { htmlCollectionOf, namedNodeMapOf, nodeListOf } from "./collections.mts";
import type { IncaElement } from "./types.mts";

type Loose = Record<PropertyKey, unknown>;

/** A stand-in element that reads `id` and the `name` attribute from a mutable record. */
function fake(attrs: { id?: string; name?: string } = {}): IncaElement {
  return {
    get id() {
      return attrs.id ?? "";
    },
    set id(value: string) {
      attrs.id = value;
    },
    getAttribute: (name: string) => (name === "name" ? (attrs.name ?? null) : null),
  } as unknown as IncaElement;
}

/** A collection over a mutable source array, with a loosely typed view of the same object. */
function make(...source: IncaElement[]) {
  const c = htmlCollectionOf(() => source);
  return { source, c, loose: c as unknown as Loose };
}

describe("a collection", () => {
  it("returns the item at an index", () => {
    const [a, b] = [fake(), fake()];
    const { c } = make(a!, b!);

    expect([c[0], c[1]]).toEqual([a, b]);
  });

  it.each(["2", "-1", "01", "1.5"])("reads undefined for the key %s outside the list", (key) => {
    const { loose } = make(fake(), fake());

    expect(loose[key]).toBeUndefined();
  });

  it("reads a non-index key as a name", () => {
    const [zero, minus] = [fake({ id: "01" }), fake({ id: "-1" })];
    const { loose } = make(zero, minus);

    expect([loose["01"], loose[-1]]).toEqual([zero, minus]);
  });

  it("has a length that follows its source", () => {
    const { c, source } = make(fake());
    const added = fake();

    source.push(added);

    expect([c.length, c[1]]).toEqual([2, added]);
  });

  it.each([
    [-1, null],
    [1.9, 1],
    [99, null],
    [4294967296, 0],
    [2 ** 32 + 1, 1],
  ])("converts the item argument %s to an unsigned long", (arg, at) => {
    const items = [fake(), fake()];
    const { c } = make(...items);

    expect(c.item(arg)).toBe(at === null ? null : items[at]);
  });

  it("throws TypeError for item without an argument", () => {
    expect(() => (make().loose["item"] as () => void)()).toThrow(TypeError);
  });

  it("throws TypeError for item with a BigInt", () => {
    expect(() => make(fake()).c.item(1n as never)).toThrow(TypeError);
  });

  it.each([
    ["spread", (c: Iterable<IncaElement>) => [...c]],
    ["Array.from", (c: Iterable<IncaElement>) => Array.from(c)],
    [
      "for...of",
      (c: Iterable<IncaElement>) => {
        const looped: IncaElement[] = [];
        for (const el of c) looped.push(el);
        return looped;
      },
    ],
    ["Array.prototype.map", (c: Iterable<IncaElement>) => Array.prototype.map.call(c, (el) => el)],
  ])("is iterable by %s", (_, read) => {
    const items = [fake(), fake()];

    expect(read(make(...items).c)).toEqual(items);
  });

  it("visits every second element when a loop removes the visited one", () => {
    const items = [fake({ id: "0" }), fake({ id: "1" }), fake({ id: "2" }), fake({ id: "3" })];
    const { c, source } = make(...items);
    const visited: string[] = [];
    for (const el of c) {
      visited.push(el.id);
      source.splice(source.indexOf(el), 1);
    }

    expect([visited, source.length]).toEqual([["0", "2"], 2]);
  });

  it("visits an element appended during a loop", () => {
    const { c, source } = make(fake({ id: "0" }));
    const visited: string[] = [];
    for (const el of c) {
      visited.push(el.id);
      if (source.length < 3) source.push(fake({ id: String(source.length) }));
    }

    expect(visited).toEqual(["0", "1", "2"]);
  });

  it("uses Array.prototype.values as its iterator", () => {
    expect(make().c[Symbol.iterator]).toBe(Array.prototype.values);
  });

  it("refuses to stop being extensible", () => {
    const { c } = make(fake());

    expect(Reflect.preventExtensions(c)).toBe(false);
    expect(() => Object.freeze(c)).toThrow(TypeError);
  });

  it("lists indices only in Object.keys", () => {
    expect(Object.keys(make(fake({ id: "box" }), fake()).c)).toEqual(["0", "1"]);
  });

  it("reports its class in toString", () => {
    expect(Object.prototype.toString.call(make().c)).toBe("[object HTMLCollection]");
  });

  it.each([
    ["a write to an index", (c: Loose) => (c[0] = 1)],
    ["a write beyond the list", (c: Loose) => (c[5] = 1)],
    ["a write to length", (c: Loose) => (c["length"] = 0)],
    ["delete of an index", (c: Loose) => delete c[0]],
  ])("throws TypeError in strict code for %s", (_, write) => {
    expect(() => write(make(fake()).loose)).toThrow(TypeError);
  });

  it("deletes an index beyond the list", () => {
    const { loose } = make(fake());

    expect(delete loose[5]).toBe(true);
  });

  it.each([
    ["an index in the list", "0", true],
    ["an index beyond the list", "5", false],
    ["length", "length", true],
    ["a member", "item", true],
    ["an unknown name", "nothing", false],
    ["the iterator", Symbol.iterator, true],
    ["an unknown Symbol", Symbol("k"), false],
  ])("reports %s through in", (_, key, expected) => {
    expect(key in make(fake()).c).toBe(expected);
  });

  it("has an unwritable, enumerable descriptor for an index", () => {
    const el = fake();

    expect(Object.getOwnPropertyDescriptor(make(el).c, "0")).toEqual({
      value: el,
      writable: false,
      enumerable: true,
      configurable: true,
    });
  });

  it("has an unwritable, hidden descriptor for a name", () => {
    const el = fake({ id: "box" });

    expect(Object.getOwnPropertyDescriptor(make(el).c, "box")).toEqual({
      value: el,
      writable: false,
      enumerable: false,
      configurable: true,
    });
  });

  it("has no own descriptor for length", () => {
    expect(Object.getOwnPropertyDescriptor(make(fake()).c, "length")).toBeUndefined();
  });

  it("has no own descriptor for a member that an id names", () => {
    expect(Object.getOwnPropertyDescriptor(make(fake({ id: "item" })).c, "item")).toBeUndefined();
  });

  it("deletes length without an error", () => {
    expect(delete make(fake()).loose["length"]).toBe(true);
  });

  it.each([
    ["an index", "0"],
    ["a supported name", "box"],
  ])("refuses defineProperty for %s", (_, key) => {
    const { c } = make(fake({ id: "box" }));

    expect(() => Object.defineProperty(c, key, { value: 1 })).toThrow(TypeError);
  });

  it("accepts defineProperty for an unknown key", () => {
    const { c, loose } = make(fake());

    Object.defineProperty(c, "foo", { value: 1, configurable: true });

    expect(loose["foo"]).toBe(1);
  });

  it("stores and deletes a Symbol key", () => {
    const { loose } = make(fake());
    const key = Symbol("k");

    loose[key] = 1;

    expect([
      loose[key],
      Object.getOwnPropertyDescriptor(loose, key)?.value,
      delete loose[key],
    ]).toEqual([1, 1, true]);
    expect(loose[key]).toBeUndefined();
  });
});

describe("a collection as a prototype", () => {
  it("creates an own property on the heir", () => {
    const { loose } = make(fake());
    const heir = Object.create(loose) as Loose;

    heir["x"] = 1;

    expect([Object.hasOwn(heir, "x"), Object.hasOwn(loose, "x")]).toEqual([true, false]);
  });

  it("creates an own index on the heir", () => {
    const { loose } = make(fake());
    const heir = Object.create(loose) as Loose;

    heir[0] = 1;

    expect([Object.hasOwn(heir, "0"), loose[0] === 1]).toEqual([true, false]);
  });

  it("throws TypeError when the heir assigns length", () => {
    const heir = Object.create(make(fake()).loose) as Loose;

    expect(() => (heir["length"] = 3)).toThrow(TypeError);
  });

  it("throws TypeError when the heir reads length", () => {
    const heir = Object.create(make(fake()).loose) as Loose;

    expect(() => heir["length"]).toThrow(TypeError);
  });

  it.each(["item", "namedItem"])("throws TypeError for %s on another receiver", (name) => {
    const method = make(fake()).loose[name] as (this: unknown, arg: string) => unknown;

    expect(() => method.call({}, "0")).toThrow(TypeError);
  });

  it("lists indices, length and methods, not names, in for...in", () => {
    const { loose } = make(fake({ id: "box" }), fake());
    const keys: string[] = [];
    for (const key in loose) keys.push(key);

    expect(keys.toSorted()).toEqual(["0", "1", "item", "length", "namedItem"]);
  });
});

describe("a collection expando", () => {
  it("lists no duplicate key when an element has the same id", () => {
    const { loose, source } = make();
    loose["box"] = 1;
    source.push(fake({ id: "box" }));

    expect(Object.getOwnPropertyNames(loose)).toEqual(["0", "box"]);
  });

  it("lists before the supported names in getOwnPropertyNames", () => {
    const { loose } = make(fake({ id: "box" }));

    loose["foo"] = 1;

    expect(Object.getOwnPropertyNames(loose)).toEqual(["0", "foo", "box"]);
  });

  it("lists a stored Symbol among the own symbols", () => {
    const { loose } = make(fake());
    const key = Symbol("k");

    loose[key] = 1;

    expect(Object.getOwnPropertySymbols(loose)).toEqual([key]);
  });

  it("shows in Object.keys after the indices", () => {
    const { loose } = make(fake(), fake());

    loose["foo"] = 1;

    expect(Object.keys(loose)).toEqual(["0", "1", "foo"]);
  });

  it("shows in getOwnPropertyNames", () => {
    const { loose } = make(fake());

    loose["foo"] = 1;

    expect(Object.getOwnPropertyNames(loose)).toEqual(["0", "foo"]);
  });

  it("shows in JSON.stringify", () => {
    const { c, loose } = make(fake());

    loose["foo"] = 1;

    expect(Object.keys(JSON.parse(JSON.stringify(c)) as object)).toEqual(["0", "foo"]);
  });

  it("shadows a name defined before the element exists", () => {
    const { c, loose, source } = make();
    Object.defineProperty(c, "box", { value: 1, configurable: true });

    source.push(fake({ id: "box" }));

    expect(loose["box"]).toBe(1);
  });

  it("reports its own descriptor in place of a supported name", () => {
    const { loose, source } = make();
    loose["box"] = 1;
    source.push(fake({ id: "box" }));

    expect(Object.getOwnPropertyDescriptor(loose, "box")).toEqual({
      value: 1,
      writable: true,
      enumerable: true,
      configurable: true,
    });
  });

  it("is deletable although an element has the name", () => {
    const { loose, source } = make();
    loose["box"] = 1;
    source.push(fake({ id: "box" }));

    expect([delete loose["box"], loose["box"]]).toEqual([true, source[0]]);
  });

  it("is assignable although an element has the name", () => {
    const { loose, source } = make();
    loose["box"] = 1;
    source.push(fake({ id: "box" }));

    loose["box"] = 2;

    expect(loose["box"]).toBe(2);
  });

  it("shadows a method by assignment", () => {
    const { loose } = make(fake());

    loose["item"] = 1;

    expect(loose["item"]).toBe(1);
  });
});

describe("namedItem", () => {
  it("finds the element with the id", () => {
    const hit = fake({ id: "box" });

    expect(make(fake(), hit).c.namedItem("box")).toBe(hit);
  });

  it("finds the element with the name", () => {
    const hit = fake({ name: "nm" });

    expect(make(fake(), hit).c.namedItem("nm")).toBe(hit);
  });

  it("prefers an id match over an earlier name match", () => {
    const byId = fake({ id: "x" });

    expect(make(fake({ name: "x" }), byId).c.namedItem("x")).toBe(byId);
  });

  it("returns the first of two id matches", () => {
    const first = fake({ id: "x" });

    expect(make(first, fake({ id: "x" })).c.namedItem("x")).toBe(first);
  });

  it("returns null for the empty string", () => {
    expect(make(fake()).c.namedItem("")).toBeNull();
  });

  it("returns null for no match", () => {
    expect(make(fake({ id: "x" })).c.namedItem("none")).toBeNull();
  });

  it("throws TypeError without an argument", () => {
    expect(() => (make().loose["namedItem"] as () => void)()).toThrow(TypeError);
  });

  it("throws TypeError for a Symbol", () => {
    expect(() => make().c.namedItem(Symbol("k") as never)).toThrow(TypeError);
  });
});

describe("a named property", () => {
  it("lists an element id that is an index only as an index", () => {
    const { loose } = make(fake({ id: "0" }));

    expect([Object.keys(loose), Object.getOwnPropertyNames(loose)]).toEqual([["0"], ["0"]]);
  });

  it("reads the element with the id", () => {
    const hit = fake({ id: "box" });

    expect(make(hit).loose["box"]).toBe(hit);
  });

  it("reads the element with the name", () => {
    const hit = fake({ name: "nm" });

    expect(make(hit).loose["nm"]).toBe(hit);
  });

  it("reads undefined for an unknown name", () => {
    expect(make(fake()).loose["nothing"]).toBeUndefined();
  });

  it("lists a repeated id once", () => {
    const { loose } = make(fake({ id: "a" }), fake({ id: "a" }));

    expect(Object.getOwnPropertyNames(loose)).toEqual(["0", "1", "a"]);
  });

  it("skips an empty id and an empty name", () => {
    const { loose } = make(fake(), fake({ name: "" }));

    expect(Object.getOwnPropertyNames(loose)).toEqual(["0", "1"]);
  });

  it("reads undefined for the empty string", () => {
    expect(make(fake()).loose[""]).toBeUndefined();
  });

  it("follows a later id change", () => {
    const el = fake();
    const { loose } = make(el);

    el.id = "box";

    expect(loose["box"]).toBe(el);
  });

  it.each([
    ["item", "function"],
    ["length", "number"],
    ["toString", "function"],
    ["constructor", "function"],
  ])("leaves the member %s in place", (name, type) => {
    expect(typeof make(fake({ id: name })).loose[name]).toBe(type);
  });

  it.each(["item", "length", "toString", "constructor"])("does not list the member %s", (name) => {
    const { loose } = make(fake({ id: name }));

    expect(Object.getOwnPropertyNames(loose).filter((key) => key === name)).toEqual([]);
  });

  it("reports a supported name through in", () => {
    expect("box" in make(fake({ id: "box" })).c).toBe(true);
  });

  it("lists indices, then names", () => {
    const { loose } = make(fake({ id: "box" }), fake({ name: "nm" }));

    expect(Object.getOwnPropertyNames(loose)).toEqual(["0", "1", "box", "nm"]);
  });

  it.each([
    ["an assignment", (c: Loose) => (c["box"] = 1)],
    ["a delete", (c: Loose) => delete c["box"]],
  ])("throws TypeError in strict code for %s", (_, write) => {
    const { loose } = make(fake({ id: "box" }));

    expect(() => write(loose)).toThrow(TypeError);
    expect(loose["box"]).toBeDefined();
  });

  it("stores an ordinary property for an unknown key", () => {
    const { loose } = make(fake());

    loose["foo"] = 1;

    expect([loose["foo"], delete loose["foo"], loose["foo"]]).toEqual([1, true, undefined]);
  });
});

describe("a NodeList", () => {
  /** A list over `items` with a loosely typed view of the same object. */
  type List = ReturnType<typeof nodeListOf>;

  function list(...items: IncaElement[]) {
    const l = nodeListOf(items);
    return { l, loose: l as unknown as Loose };
  }

  it("reads an element id as undefined", () => {
    expect(list(fake({ id: "box" })).loose["box"]).toBeUndefined();
  });

  it("reports its class in toString", () => {
    expect(Object.prototype.toString.call(list().l)).toBe("[object NodeList]");
  });

  it("keeps its length after the source array changes", () => {
    const items = [fake()];
    const l = nodeListOf(items);

    items.push(fake());

    expect(l.length).toBe(1);
  });

  it.each([
    [0, 0],
    [1, 1],
    [2, null],
    [-1, null],
  ])("item(%s) reads the entry or null", (index, at) => {
    const items = [fake(), fake()];
    const l = nodeListOf(items);

    expect(l.item(index)).toBe(at === null ? null : items[at]);
  });

  it("forEach passes value, index, list and thisArg", () => {
    const [a, b] = [fake(), fake()];
    const { l } = list(a!, b!);
    const calls: unknown[][] = [];
    const that = {};

    l.forEach(function (this: unknown, ...args) {
      calls.push([this, ...args]);
    }, that);

    expect(calls[0]?.[0]).toBe(that);
    expect(calls.map(([, value, index]) => [value, index])).toEqual([
      [a, 0],
      [b, 1],
    ]);
    expect(calls[1]?.[3]).toBe(l);
  });

  it.each([
    ["the indices from keys", (l: List) => [...l.keys()], () => [0, 1]],
    ["the items from values", (l: List) => [...l.values()], (e: IncaElement[]) => e],
    [
      "the pairs from entries",
      (l: List) => [...l.entries()],
      (e: IncaElement[]) => e.map((el, i) => [i, el]),
    ],
  ])("lists %s", (_, read, expected) => {
    const items = [fake(), fake()];
    const { l } = list(...items);

    expect(read(l)).toEqual(expected(items));
  });

  it("uses values as its iterator", () => {
    const { loose } = list(fake());

    expect(loose[Symbol.iterator]).toBe(loose["values"]);
  });

  it("visits every element of a loop that removes elements from its source", () => {
    const items = [fake(), fake(), fake()];
    const l = nodeListOf(items);
    const visited: unknown[] = [];

    for (const el of l) {
      visited.push(el);
      items.length = 0;
    }

    expect([visited.length, l.length]).toEqual([3, 3]);
  });

  it.each(["forEach", "entries", "keys", "values"])("takes %s from Array.prototype", (name) => {
    expect(list().loose[name]).toBe((Array.prototype as unknown as Loose)[name]);
  });

  it("throws TypeError for item on another receiver", () => {
    const item = list().loose["item"] as (this: unknown, index: number) => unknown;

    expect(() => item.call({}, 0)).toThrow(TypeError);
  });

  it("throws TypeError for the length getter on another receiver", () => {
    const heir = Object.create(list(fake()).loose) as Loose;

    expect(() => heir["length"]).toThrow(TypeError);
  });

  it("reports the length of item", () => {
    expect((list().loose["item"] as () => void).length).toBe(1);
  });

  it.each(["item", "box"])("stores %s as an own property", (name) => {
    const { loose } = list(fake({ id: "box" }));

    loose[name] = 1;

    expect([loose[name], Object.hasOwn(loose, name)]).toEqual([1, true]);
  });

  it.each([
    ["an index assignment", (l: Loose) => (l[0] = 1)],
    ["a length assignment", (l: Loose) => (l["length"] = 0)],
    ["an index delete", (l: Loose) => delete l[0]],
  ])("throws TypeError in strict code for %s", (_, write) => {
    const { loose } = list(fake());

    expect(() => write(loose)).toThrow(TypeError);
    expect(loose["length"]).toBe(1);
  });

  it("lists the indices and an expando, not length", () => {
    const { loose } = list(fake(), fake());

    loose["foo"] = 1;

    expect([Object.getOwnPropertyNames(loose), Object.keys(loose)]).toEqual([
      ["0", "1", "foo"],
      ["0", "1", "foo"],
    ]);
  });

  it.each(["forEach", "entries", "keys", "values"])("gives %s to a NodeList only", (name) => {
    const { c } = make();
    const map = namedNodeMapOf(
      () => [],
      () => null,
      () => null,
    );

    expect([name in c, name in map, name in nodeListOf([])]).toEqual([false, false, true]);
  });
});
