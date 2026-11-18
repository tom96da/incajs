// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { adoptElement } from "../../tests/adopt.mts";
import { nativeStub } from "../../tests/setup.mts";
import { prototypes } from "./lifecycle.mts";
import type { Element } from "./types.mts";

/** The members of the draft IDL that hold elements, with the attribute each reflects. */
const MEMBERS: [string, string][] = [
  ["ariaActiveDescendantElement", "aria-activedescendant"],
  ["ariaControlsElements", "aria-controls"],
  ["ariaDescribedByElements", "aria-describedby"],
  ["ariaDetailsElements", "aria-details"],
  ["ariaErrorMessageElements", "aria-errormessage"],
  ["ariaFlowToElements", "aria-flowto"],
  ["ariaLabelledByElements", "aria-labelledby"],
  ["ariaOwnsElements", "aria-owns"],
];

/** Installs a fake host that keeps the ids of each attribute, and returns it with its elements. */
function setup() {
  const store = new Map<string, number[] | null>();
  const host = {
    ariaElements: vi.fn<(id: number, attribute: string) => number[] | null>(
      (id, attribute) => store.get(`${id}:${attribute}`) ?? null,
    ),
    setAriaElements: vi.fn<(id: number, attribute: string, ids: number[] | null) => void>(
      (id, attribute, ids) => void store.set(`${id}:${attribute}`, ids),
    ),
  };
  vi.stubGlobal("__inca_native__", nativeStub(host as never));
  return { host, store, owner: adoptElement(1), a: adoptElement(2), b: adoptElement(3) };
}

const lists = MEMBERS.filter(([name]) => name.endsWith("Elements"));

describe("the element members of the ARIA table", () => {
  it.each(MEMBERS)("%s is an enumerable, configurable accessor of Element only", (name) => {
    const all = prototypes() as unknown as Record<string, object>;
    const descriptor = Object.getOwnPropertyDescriptor(all.Element!, name)!;

    expect([typeof descriptor.get, typeof descriptor.set]).toEqual(["function", "function"]);
    expect([descriptor.enumerable, descriptor.configurable]).toEqual([true, true]);
    expect(Object.hasOwn(all.HTMLElement!, name)).toBe(false);
  });

  it.each(MEMBERS)("%s reads the attribute the host names", (name, attribute) => {
    const { host, owner } = setup();

    Reflect.get(owner, name);

    expect(host.ariaElements).toHaveBeenCalledWith(1, attribute);
  });
});

describe("the single element member", () => {
  it("maps the id to the handle", () => {
    const { store, owner, a } = setup();
    store.set("1:aria-activedescendant", [2]);

    expect(owner.ariaActiveDescendantElement).toBe(a);
  });

  it.each([
    ["null", null],
    ["an empty list", []],
  ])("reads null for %s", (_name, ids) => {
    const { store, owner } = setup();
    store.set("1:aria-activedescendant", ids);

    expect(owner.ariaActiveDescendantElement).toBeNull();
  });

  it("sends the id of an element to the host", () => {
    const { host, owner, a } = setup();

    owner.ariaActiveDescendantElement = a;

    expect(host.setAriaElements).toHaveBeenCalledExactlyOnceWith(1, "aria-activedescendant", [2]);
  });

  it.each([null, undefined])("removes the reference for %s", (value) => {
    const { host, owner } = setup();

    owner.ariaActiveDescendantElement = value as unknown as null;

    expect(host.setAriaElements).toHaveBeenCalledExactlyOnceWith(1, "aria-activedescendant", null);
  });

  it("throws a TypeError for an object that is no element", () => {
    const { host, owner } = setup();

    expect(() => {
      owner.ariaActiveDescendantElement = {} as unknown as Element;
    }).toThrow(new TypeError("the value is not an Element. pass an element or null"));
    expect(host.setAriaElements).not.toHaveBeenCalled();
  });
});

describe("the list members", () => {
  it("map the ids to handles in order", () => {
    const { store, owner, a, b } = setup();
    store.set("1:aria-owns", [3, 2]);

    expect(owner.ariaOwnsElements).toEqual([b, a]);
  });

  it("return a frozen array", () => {
    const { store, owner } = setup();
    store.set("1:aria-owns", [2]);

    expect(Object.isFrozen(owner.ariaOwnsElements)).toBe(true);
  });

  it("return the same array while the nodes stay the same", () => {
    const { store, owner } = setup();
    store.set("1:aria-owns", [2, 3]);

    expect(owner.ariaOwnsElements).toBe(owner.ariaOwnsElements);
  });

  it("return a new array once the resolution changes", () => {
    const { store, owner } = setup();
    store.set("1:aria-owns", [2]);
    const first = owner.ariaOwnsElements;
    store.set("1:aria-owns", [3]);

    expect(owner.ariaOwnsElements === first).toBe(false);
  });

  it("return the first array after the same elements are set twice", () => {
    const { owner, a } = setup();
    owner.ariaOwnsElements = [a];
    const first = owner.ariaOwnsElements;

    owner.ariaOwnsElements = [a];
    owner.ariaOwnsElements = [a];

    expect(owner.ariaOwnsElements).toBe(first);
  });

  it("return a new array after a null read, whatever value is restored", () => {
    const { store, owner } = setup();
    store.set("1:aria-owns", [2]);
    const first = owner.ariaOwnsElements;
    store.set("1:aria-owns", null);
    expect(owner.ariaOwnsElements).toBeNull();
    store.set("1:aria-owns", [2]);

    expect(owner.ariaOwnsElements === first).toBe(false);
  });

  it.each(lists)("%s sends the ids of an iterable in order", (name, attribute) => {
    const { host, owner, a, b } = setup();

    Reflect.set(owner, name, new Set([b, a]));

    expect(host.setAriaElements).toHaveBeenCalledExactlyOnceWith(1, attribute, [3, 2]);
  });

  it.each([null, undefined])("remove the reference for %s", (value) => {
    const { host, owner } = setup();

    owner.ariaOwnsElements = value as unknown as null;

    expect(host.setAriaElements).toHaveBeenCalledExactlyOnceWith(1, "aria-owns", null);
  });

  it.each([
    ["a number", 5, "the value is not a sequence of Element. pass an array of elements or null"],
    ["a string", "a", "the value is not a sequence of Element. pass an array of elements or null"],
    [
      "a plain object",
      {},
      "the value is not a sequence of Element. pass an array of elements or null",
    ],
    ["an array with null", [null], "the value is not an Element. pass an element or null"],
    ["an array with a number", [1], "the value is not an Element. pass an element or null"],
  ])("throw a TypeError for %s and leave the host alone", (_name, value, message) => {
    const { host, owner } = setup();

    expect(() => {
      owner.ariaOwnsElements = value as unknown as Element[];
    }).toThrow(new TypeError(message));
    expect(host.setAriaElements).not.toHaveBeenCalled();
  });

  it("throw the element message for a text node in the array", () => {
    const { host, owner } = setup();

    expect(() => {
      owner.ariaOwnsElements = [{ nodeType: 3 }] as unknown as Element[];
    }).toThrow(new TypeError("the value is not an Element. pass an element or null"));
    expect(host.setAriaElements).not.toHaveBeenCalled();
  });
});
