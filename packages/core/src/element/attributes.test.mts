// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { adoptElement } from "../../tests/adopt.mts";
import { nativeStub } from "../../tests/setup.mts";
import { markFreed, markRemoved } from "./lifecycle.mts";
import type { ElementName } from "../ffi/index.mts";
import type { Element } from "./types.mts";

const SVG_NAMESPACE = "http://www.w3.org/2000/svg";
const SVG: ElementName = ["svg", SVG_NAMESPACE, null, "svg"];

type Stored = string | number | boolean;
type Loose = Record<PropertyKey, unknown>;

/** Folds A to Z only. */
const fold = (key: string): string => key.replace(/[A-Z]/g, (letter) => letter.toLowerCase());

/**
 * Installs a fake host that keeps attributes in a map, and returns it with a handle. An HTML
 * element folds the keys and any other element keeps them.
 */
function setup(initial: Record<string, Stored> = {}, name: ElementName = "div") {
  const store = new Map<string, Stored>(Object.entries(initial));
  const keyOf = (key: string): string => (typeof name === "string" ? fold(key) : key);
  const host = {
    attributeKey: vi.fn<(nodeId: number, name: string) => string>((_id, key) => keyOf(key)),
    getAttribute: vi.fn<(nodeId: number, key: string) => Stored | null>(
      (_id, key) => store.get(keyOf(key)) ?? null,
    ),
    setAttribute: vi.fn<(nodeId: number, key: string, value: Stored) => void>((_id, key, value) => {
      store.set(keyOf(key), value);
    }),
    removeAttribute: vi.fn<(nodeId: number, key: string) => void>((_id, key) => {
      store.delete(keyOf(key));
    }),
    attributeNames: vi.fn<(nodeId: number) => string[]>(() => [...store.keys()]),
  };
  vi.stubGlobal("__inca_native__", nativeStub(host as never));
  const el: Element = adoptElement(1, name);
  return { host, el, store };
}

describe("attributes", () => {
  it("lists the attributes in the order of the host", () => {
    const { el } = setup({ id: "x", b: 1, a: 2 });

    expect([el.attributes[0]?.name, el.attributes[1]?.name, el.attributes[2]?.name]).toEqual([
      "id",
      "b",
      "a",
    ]);
  });

  it("has a length that follows setAttribute and removeAttribute", () => {
    const { el } = setup({ a: "1" });
    const lengths = [el.attributes.length];

    el.setAttribute("b", "2");
    lengths.push(el.attributes.length);
    el.removeAttribute("a");
    lengths.push(el.attributes.length);

    expect(lengths).toEqual([1, 2, 1]);
  });

  it("returns the same object on every read", () => {
    const { el } = setup();

    expect(el.attributes).toBe(el.attributes);
  });

  it("iterates Attr objects in order", () => {
    const { el } = setup({ b: "2", a: "1" });
    const seen: [string, string][] = [];
    for (const attr of el.attributes) seen.push([attr.name, attr.value]);

    expect(seen).toEqual([
      ["b", "2"],
      ["a", "1"],
    ]);
    expect(el.attributes[Symbol.iterator]().next().value).toBe(el.attributes.getNamedItem("b"));
  });

  it("visits every second attribute when a loop removes the visited one", () => {
    const { el } = setup({ a: "1", b: "2", c: "3", d: "4" });
    const visited: string[] = [];
    for (const attr of el.attributes) {
      visited.push(attr.name);
      el.removeAttribute(attr.name);
    }

    expect([visited, el.attributes.length]).toEqual([["a", "c"], 2]);
  });

  it("returns an Attr object from an index", () => {
    const { el } = setup({ a: "1" });

    expect(el.attributes[0]).toBe(el.attributes.item(0));
  });

  it("reports its class in toString", () => {
    expect(Object.prototype.toString.call(setup().el.attributes)).toBe("[object NamedNodeMap]");
  });

  it("lists indices only in Object.keys", () => {
    expect(Object.keys(setup({ "data-x": "1" }).el.attributes)).toEqual(["0"]);
  });

  it("lists indices, expandos, then names", () => {
    const { el } = setup({ "data-x": "1", id: "y" });
    (el.attributes as unknown as Loose)["foo"] = 1;

    expect(Object.getOwnPropertyNames(el.attributes)).toEqual(["0", "1", "foo", "data-x", "id"]);
  });

  it("does not list length as an own key", () => {
    expect(Object.hasOwn(setup().el.attributes, "length")).toBe(false);
  });

  it("reads a named property", () => {
    const { el } = setup({ "data-x": "1" });
    const loose = el.attributes as unknown as Loose;

    expect([loose["data-x"], "data-x" in el.attributes]).toEqual([
      el.attributes.getNamedItem("data-x"),
      true,
    ]);
  });

  it("reads the attribute of an HTML element through a name with an upper case letter", () => {
    const { el } = setup({ viewbox: "0 0 1 1" });

    expect((el.attributes as unknown as Loose)["viewBox"]).toBe(
      el.attributes.getNamedItem("viewbox"),
    );
  });

  it("reads the attribute of an SVG element through its own case only", () => {
    const { el } = setup({ viewBox: "0 0 1 1" }, SVG);
    const map = el.attributes as unknown as Loose;

    expect([map["viewBox"], map["viewbox"]]).toEqual([
      el.attributes.getNamedItem("viewBox"),
      undefined,
    ]);
  });

  it("reads undefined for an unset name", () => {
    expect((setup().el.attributes as unknown as Loose)["data-x"]).toBeUndefined();
  });

  it("follows a later setAttribute in a name", () => {
    const { el } = setup();
    const map = el.attributes as unknown as Loose;

    el.setAttribute("data-y", "1");

    expect((map["data-y"] as { value: string }).value).toBe("1");
  });

  it.each(["length", "item"])("leaves the member in place for an attribute named %s", (name) => {
    const { el } = setup({ [name]: "1" });

    expect(typeof (el.attributes as unknown as Loose)[name]).toBe(
      name === "length" ? "number" : "function",
    );
  });

  it.each([
    ["an assignment", (map: Loose) => (map["data-x"] = 1)],
    ["a delete", (map: Loose) => delete map["data-x"]],
  ])("throws TypeError in strict code for %s", (_, write) => {
    const { el } = setup({ "data-x": "1" });

    expect(() => write(el.attributes as unknown as Loose)).toThrow(TypeError);
  });

  it("stores an ordinary property for an unknown key", () => {
    const map = setup().el.attributes as unknown as Loose;

    map["foo"] = 1;

    expect([map["foo"], delete map["foo"], map["foo"]]).toEqual([1, true, undefined]);
  });

  it("returns null from item outside the list", () => {
    expect(setup({ a: "1" }).el.attributes.item(1)).toBeNull();
  });
});

describe("getNamedItem", () => {
  it("finds a name written in another case", () => {
    const attr = setup({ id: "x" }).el.attributes.getNamedItem("ID");

    expect([attr?.name, attr?.value]).toEqual(["id", "x"]);
  });

  it("leaves a non-ASCII letter unfolded", () => {
    const { el } = setup({ "data-é": "1" });

    expect([el.attributes.getNamedItem("DATA-É"), el.attributes.getNamedItem("DATA-é")]).toEqual([
      null,
      expect.objectContaining({ name: "data-é" }),
    ]);
  });

  it("asks the host for the key of the name", () => {
    const { el, host } = setup({ id: "x" });

    el.attributes.getNamedItem("ID");

    expect(host.attributeKey).toHaveBeenCalledWith(1, "ID");
    expect(host.getAttribute).toHaveBeenCalledWith(1, "id");
  });

  it("keeps the case of a name on an SVG element", () => {
    const { el } = setup({ viewBox: "0 0 1 1" }, SVG);

    expect([
      el.attributes.getNamedItem("viewBox")?.name,
      el.attributes.getNamedItem("viewbox"),
    ]).toEqual(["viewBox", null]);
  });

  it("returns null for an unset attribute", () => {
    expect(setup().el.attributes.getNamedItem("a")).toBeNull();
  });

  it("throws TypeError without an argument", () => {
    const { el } = setup();

    expect(() => (el.attributes.getNamedItem as unknown as () => void)()).toThrow(TypeError);
  });

  it("throws TypeError for a Symbol", () => {
    expect(() => setup().el.attributes.getNamedItem(Symbol("k") as never)).toThrow(TypeError);
  });

  it("converts a number to text", () => {
    expect(setup({ "1": "x" }).el.attributes.getNamedItem(1 as never)?.value).toBe("x");
  });
});

describe("an Attr", () => {
  it("reports its name", () => {
    const attr = setup({ id: "x" }).el.attributes.getNamedItem("id")!;

    expect([attr.name, attr.localName]).toEqual(["id", "id"]);
  });

  it("reports its owner", () => {
    const { el } = setup({ id: "x" });
    const attr = el.attributes.getNamedItem("id")!;

    expect(attr.ownerElement).toBe(el);
  });

  it("reports the fixed namespace members", () => {
    const attr = setup({ id: "x" }).el.attributes.getNamedItem("id")!;

    expect([attr.namespaceURI, attr.prefix, attr.specified]).toEqual([null, null, true]);
  });

  it("reports its class in toString", () => {
    const attr = setup({ id: "x" }).el.attributes.getNamedItem("id")!;

    expect(Object.prototype.toString.call(attr)).toBe("[object Attr]");
  });

  it("reads the live value", () => {
    const { el } = setup({ id: "x" });
    const attr = el.attributes.getNamedItem("id")!;

    el.setAttribute("id", "y");

    expect(attr.value).toBe("y");
  });

  it("is the same object while the attribute is set", () => {
    const { el } = setup({ id: "x" });

    expect(el.attributes.getNamedItem("id")).toBe(el.attributes.getNamedItem("id"));
  });

  it("is the same object from an index and from a name", () => {
    const { el } = setup({ id: "x" });

    expect(el.attributes[0]).toBe(el.attributes.getNamedItem("id"));
  });

  it("keeps its last value when the attribute is set again", () => {
    const { el } = setup({ id: "x" });
    const old = el.attributes.getNamedItem("id")!;
    el.removeAttribute("id");
    el.attributes.getNamedItem("id");
    el.setAttribute("id", "z");

    expect(old.value).toBe("x");
  });

  it("is a new object when the attribute is set again after a removal", () => {
    const { el } = setup({ id: "x" });
    const old = el.attributes.getNamedItem("id");

    el.removeAttribute("id");
    el.attributes.getNamedItem("id");
    el.setAttribute("id", "z");

    expect(el.attributes.getNamedItem("id")).not.toBe(old);
  });

  it.each([
    ["a string", "2" as unknown, "2"],
    ["a number", 1 as unknown, "1"],
  ])("sets the attribute when %s is assigned to value", (_, value, text) => {
    const { el, host } = setup({ id: "x" });

    el.attributes.getNamedItem("id")!.value = value as never;

    expect(el.getAttribute("id")).toBe(text);
    expect(host.setAttribute).toHaveBeenCalledWith(1, "id", text);
  });

  it("throws TypeError when a Symbol is assigned to value", () => {
    const attr = setup({ id: "x" }).el.attributes.getNamedItem("id")!;

    expect(() => (attr.value = Symbol("k") as never)).toThrow(TypeError);
  });

  it("has no owner after a removal and keeps the last value", () => {
    const { el } = setup({ id: "x" });
    const attr = el.attributes.getNamedItem("id")!;

    el.removeAttribute("id");

    expect([attr.ownerElement, attr.name, attr.value]).toEqual([null, "id", "x"]);
  });

  it("leaves the element unset when value is assigned after a removal", () => {
    const { el } = setup({ id: "x" });
    const attr = el.attributes.getNamedItem("id")!;
    el.removeAttribute("id");

    attr.value = "2";

    expect([attr.value, el.getAttribute("id")]).toEqual(["2", null]);
  });

  it("detaches after a removal through the host", () => {
    const { el, store } = setup({ id: "x" });
    const attr = el.attributes.getNamedItem("id")!;

    store.delete("id");

    expect(attr.ownerElement).toBeNull();
  });
});

describe("a freed node", () => {
  it("gives an empty attributes map without a host call", () => {
    const { el, host } = setup({ a: "1" });
    markFreed([el.__node_id]);

    expect(el.attributes.length).toBe(0);
    expect(host.attributeNames).not.toHaveBeenCalled();
  });

  it("answers null from getNamedItem without a host call", () => {
    const { el, host } = setup({ a: "1" });
    markFreed([el.__node_id]);

    expect(el.attributes.getNamedItem("a")).toBeNull();
    expect(host.getAttribute).not.toHaveBeenCalled();
    expect(host.attributeKey).not.toHaveBeenCalled();
  });

  it("throws TypeError from getNamedItem without an argument", () => {
    const { el } = setup({ a: "1" });
    markFreed([el.__node_id]);

    expect(() => (el.attributes.getNamedItem as unknown as () => void)()).toThrow(TypeError);
  });

  it("reads undefined for a name without a host call", () => {
    const { el, host } = setup({ a: "1" });
    markFreed([el.__node_id]);

    expect((el.attributes as unknown as Loose)["a"]).toBeUndefined();
    expect(host.getAttribute).not.toHaveBeenCalled();
    expect(host.attributeKey).not.toHaveBeenCalled();
  });

  it("detaches a held Attr", () => {
    const { el } = setup({ a: "1" });
    const attr = el.attributes.getNamedItem("a")!;
    markFreed([el.__node_id]);

    expect(attr.ownerElement).toBeNull();
  });

  it("ignores an assignment to value of a held Attr", () => {
    const { el, host } = setup({ a: "1" });
    const attr = el.attributes.getNamedItem("a")!;
    markFreed([el.__node_id]);

    attr.value = "2";

    expect(host.setAttribute).not.toHaveBeenCalled();
  });
});

describe("a removed node", () => {
  /** A removed node over a host that holds `a`. */
  function removed() {
    const { el } = setup({ a: "1" });
    markRemoved([el.__node_id]);
    return el;
  }

  it("stores a written attribute", () => {
    const el = removed();

    el.setAttribute("b", "2");

    expect(el.getAttribute("b")).toBe("2");
  });

  it("removes an attribute", () => {
    const el = removed();

    el.removeAttribute("a");

    expect(el.hasAttribute("a")).toBe(false);
  });

  it("lists its attributes", () => {
    expect(Array.from(removed().attributes, (attr) => [attr.name, attr.value])).toEqual([
      ["a", "1"],
    ]);
  });
});
