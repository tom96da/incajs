// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import { createIncaElement } from "./handle.mts";
import type { DOMStringMap } from "./types.mts";

/** Stands for the prototype the host gives the target. */
class FakeDOMStringMap {
  get [Symbol.toStringTag](): string {
    return "DOMStringMap";
  }
}

/** Installs a fake host that keeps dataset keys in a map, and returns it with the element. */
function setup(initial: Record<string, string> = {}) {
  const store = new Map<string, string>(Object.entries(initial));
  const host = {
    setAttribute: vi.fn<(nodeId: number, key: string, value: string) => void>((_id, key, value) => {
      store.set(key.replace("data-", ""), value);
    }),
    datasetGet: vi.fn<(nodeId: number, key: string) => string | undefined>((_id, key) =>
      store.get(key),
    ),
    datasetSet: vi.fn<(nodeId: number, key: string, value: string) => void>((_id, key, value) => {
      store.set(key, value);
    }),
    datasetDelete: vi.fn<(nodeId: number, key: string) => boolean>((_id, key) => {
      store.delete(key);
      return true;
    }),
    datasetKeys: vi.fn<(nodeId: number) => string[]>(() => [...store.keys()]),
    datasetTarget: vi.fn<() => object>(() => new FakeDOMStringMap()),
  };
  globalThis.__inca_native__ = nativeStub(host as never);
  const el = createIncaElement(1, "element", "div");
  const dataset: DOMStringMap = el.dataset;
  return { host, el, dataset, store };
}

/** The dataset typed for Symbol keys. */
const symbols = (dataset: DOMStringMap) => dataset as Record<symbol, unknown>;

describe("dataset", () => {
  it("is the same object on every read", () => {
    const { el } = setup();

    expect(el.dataset).toBe(el.dataset);
  });

  it("leaves a comment stand-in without dataset", () => {
    setup();

    expect("dataset" in createIncaElement(8, "comment")).toBe(false);
  });

  it("reads the text the host answers", () => {
    expect(setup({ fooBar: "3" }).dataset.fooBar).toBe("3");
  });

  it("reads an empty value as an empty string", () => {
    expect(setup({ a: "" }).dataset.a).toBe("");
  });

  it("reads an absent key as undefined", () => {
    expect(setup().dataset.missing).toBeUndefined();
  });

  it("reads a key set after the first read", () => {
    const { dataset, store } = setup();
    void dataset.later;
    store.set("later", "x");

    expect(dataset.later).toBe("x");
  });

  it("reads back a written value", () => {
    const { dataset } = setup();

    dataset.n = "v";

    expect(dataset.n).toBe("v");
  });

  it("writes the text of the value", () => {
    const { host, dataset } = setup();

    dataset.n = 3 as unknown as string;

    expect(host.datasetSet).toHaveBeenCalledExactlyOnceWith(1, "n", "3");
  });

  it("writes the text null for null", () => {
    const { host, dataset } = setup();

    dataset.n = null as unknown as string;

    expect(host.datasetSet).toHaveBeenCalledExactlyOnceWith(1, "n", "null");
  });

  it("writes nothing for a Symbol value", () => {
    const { host, dataset } = setup();

    expect(() => {
      dataset.n = Symbol("s") as unknown as string;
    }).toThrow(TypeError);
    expect(host.datasetSet).not.toHaveBeenCalled();
  });

  it.each(["SyntaxError", "InvalidCharacterError"])(
    "propagates the %s the host throws for a write",
    (name) => {
      const { host, dataset } = setup();
      host.datasetSet.mockImplementation(() => {
        throw new DOMException("refused", name);
      });

      expect(() => {
        dataset.n = "v";
      }).toThrow(expect.objectContaining({ name }));
    },
  );

  it("returns true from a strict-mode delete", () => {
    expect(delete setup({ a: "1" }).dataset.a).toBe(true);
  });

  it("asks the host to delete the key", () => {
    const { host, dataset } = setup({ a: "1" });

    delete dataset.a;

    expect(host.datasetDelete).toHaveBeenCalledExactlyOnceWith(1, "a");
  });

  it("lists the keys in the order the host gives", () => {
    expect(Object.keys(setup({ b: "1", a: "2" }).dataset)).toEqual(["b", "a"]);
  });

  it("lists the keys with for-in", () => {
    const keys: string[] = [];

    for (const key in setup({ b: "1", a: "2" }).dataset) keys.push(key);

    expect(keys).toEqual(["b", "a"]);
  });

  it("serializes the keys with JSON.stringify", () => {
    expect(JSON.stringify(setup({ b: "1", a: "2" }).dataset)).toBe('{"b":"1","a":"2"}');
  });

  it("reports a listed key with in", () => {
    expect("a" in setup({ a: "1" }).dataset).toBe(true);
  });

  it("reports an unlisted key as absent with in", () => {
    expect("a" in setup().dataset).toBe(false);
  });

  it("reports a prototype member with in", () => {
    expect("toString" in setup().dataset).toBe(true);
  });

  it("reads constructor from the prototype", () => {
    expect(setup().dataset.constructor).toBe(FakeDOMStringMap);
  });

  it("is an instance of the prototype's class", () => {
    expect(setup().dataset instanceof FakeDOMStringMap).toBe(true);
  });

  it("reaches hasOwnProperty on the prototype chain", () => {
    expect(setup({ x: "1" }).dataset.hasOwnProperty("x")).toBe(true);
  });

  it("reads toString from the prototype", () => {
    const { dataset } = setup();

    expect(Reflect.get(dataset, "toString")).toBe(Reflect.get(Object.prototype, "toString"));
  });

  it("reads the tag of the prototype through Object.prototype.toString", () => {
    const { dataset } = setup();

    expect(Object.prototype.toString.call(dataset)).toBe("[object DOMStringMap]");
  });

  it("shadows a prototype member with a written key", () => {
    const { dataset } = setup();

    Reflect.set(dataset, "constructor", "x");

    expect(dataset.constructor).toBe("x");
  });

  it("reads Symbol.iterator as undefined", () => {
    expect(symbols(setup().dataset)[Symbol.iterator]).toBeUndefined();
  });

  it("describes a listed key as writable, enumerable and configurable", () => {
    const { dataset } = setup({ a: "1" });

    expect(Object.getOwnPropertyDescriptor(dataset, "a")).toEqual({
      value: "1",
      writable: true,
      enumerable: true,
      configurable: true,
    });
  });

  it("describes an absent key as undefined", () => {
    expect(Object.getOwnPropertyDescriptor(setup().dataset, "missing")).toBeUndefined();
  });

  it("writes data-x through defineProperty with a value", () => {
    const { host, dataset } = setup();

    Object.defineProperty(dataset, "x", { value: 7 });

    expect(host.datasetSet).toHaveBeenCalledExactlyOnceWith(1, "x", "7");
  });

  it("throws a TypeError for defineProperty with an accessor", () => {
    const { host, dataset } = setup();

    expect(() => Object.defineProperty(dataset, "x", { get: () => "v" })).toThrow(TypeError);
    expect(host.datasetSet).not.toHaveBeenCalled();
  });

  it("throws a TypeError for defineProperty with configurable false and writes nothing", () => {
    const { host, dataset } = setup();

    expect(() => Object.defineProperty(dataset, "x", { value: "v", configurable: false })).toThrow(
      TypeError,
    );
    expect(host.datasetSet).not.toHaveBeenCalled();
  });

  it("writes data-x through defineProperty with configurable true", () => {
    const { host, dataset } = setup();

    Object.defineProperty(dataset, "x", { value: "v", configurable: true });

    expect(host.datasetSet).toHaveBeenCalledExactlyOnceWith(1, "x", "v");
  });

  it("defines a Symbol key on the target", () => {
    const { host, dataset } = setup();
    const key = Symbol("k");

    Object.defineProperty(dataset, key, { value: 1, configurable: true });

    expect(symbols(dataset)[key]).toBe(1);
    expect(host.datasetSet).not.toHaveBeenCalled();
  });

  it("throws a TypeError for Object.preventExtensions", () => {
    expect(() => Object.preventExtensions(setup().dataset)).toThrow(TypeError);
  });

  it("lists the keys after a refused freeze", () => {
    const { dataset } = setup({ a: "1" });
    expect(() => Object.freeze(dataset)).toThrow(TypeError);

    expect(Object.keys(dataset)).toEqual(["a"]);
  });

  it("keeps a written Symbol key on the target", () => {
    const { host, dataset } = setup();
    const key = Symbol("k");

    symbols(dataset)[key] = 5;

    expect(host.datasetSet).not.toHaveBeenCalled();
  });

  it("reads a written Symbol key", () => {
    const { dataset } = setup();
    const key = Symbol("k");
    symbols(dataset)[key] = 5;

    expect(symbols(dataset)[key]).toBe(5);
  });

  it("lists a Symbol key after the dataset keys", () => {
    const { dataset } = setup({ a: "1" });
    const key = Symbol("k");
    symbols(dataset)[key] = 5;

    expect(Reflect.ownKeys(dataset)).toEqual(["a", key]);
  });

  it("deletes a Symbol key", () => {
    const { dataset } = setup();
    const key = Symbol("k");
    symbols(dataset)[key] = 5;

    expect(delete symbols(dataset)[key]).toBe(true);
    expect(key in dataset).toBe(false);
  });

  it("leaves the host alone when a Symbol key is deleted", () => {
    const { host, dataset } = setup();
    const key = Symbol("k");
    symbols(dataset)[key] = 5;

    delete symbols(dataset)[key];

    expect(host.datasetDelete).not.toHaveBeenCalled();
  });

  it("throws a TypeError for a strict-mode write to the member", () => {
    const { el, dataset } = setup();

    expect(() => {
      (el as { dataset: unknown }).dataset = dataset;
    }).toThrow(TypeError);
  });
});
