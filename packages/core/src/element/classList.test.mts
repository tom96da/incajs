// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import { createIncaElement } from "./handle.mts";
import { markDestroyed } from "./lifecycle.mts";
import type { IncaElement } from "./types.mts";

type Stored = string | number | boolean;

/** Installs a fake host with one attribute map per node, and returns it with a handle. */
function setup(initial: Record<string, Stored> = {}, kind: IncaElement["kind"] = "element") {
  const store = new Map<string, Stored>(Object.entries(initial));
  const lists: object[] = [];
  const host = {
    getAttribute: vi.fn<(nodeId: number, key: string) => Stored | null>(
      (_id, key) => store.get(key) ?? null,
    ),
    setAttribute: vi.fn<(nodeId: number, key: string, value: Stored) => void>((_id, key, value) => {
      store.set(key, value);
    }),
    classList: vi.fn<(nodeId: number) => object>(() => {
      const list = { value: "" };
      lists.push(list);
      return list;
    }),
  };
  globalThis.__inca_native__ = nativeStub(host as never);
  const el: IncaElement = createIncaElement(1, kind, "div");
  return { host, el, store, lists };
}

describe("className", () => {
  it("reads an empty string when the attribute is absent", () => {
    expect(setup().el.className).toBe("");
  });

  it("reads the text of the class attribute", () => {
    expect(setup({ class: "a  b" }).el.className).toBe("a  b");
  });

  it("reads a number attribute as its text", () => {
    expect(setup({ class: 5 }).el.className).toBe("5");
  });

  it.each([
    [null, "null"],
    [undefined, "undefined"],
    [5, "5"],
  ])("writes %s as the text %s", (value, text) => {
    const { el, host } = setup();

    el.className = value as unknown as string;

    expect(host.setAttribute).toHaveBeenCalledWith(1, "class", text);
  });

  it("throws a TypeError when a symbol is written", () => {
    const { el } = setup();

    expect(() => {
      el.className = Symbol("c") as unknown as string;
    }).toThrow(TypeError);
  });

  it("throws a TypeError when a symbol is written to a freed element", () => {
    const { el } = setup();
    markDestroyed(el);

    expect(() => {
      el.className = Symbol("c") as unknown as string;
    }).toThrow(TypeError);
  });

  it("reads an empty string on a freed element", () => {
    const { el } = setup({ class: "a" });
    markDestroyed(el);

    expect(el.className).toBe("");
  });

  it("ignores a write on a freed element", () => {
    const { el, host } = setup();
    markDestroyed(el);

    el.className = "a";

    expect(host.setAttribute).not.toHaveBeenCalled();
  });
});

describe("classList", () => {
  it("returns the same list on each read", () => {
    const { el } = setup();

    expect(el.classList).toBe(el.classList);
  });

  it("writes the value of the list when a string is assigned", () => {
    const { el, lists } = setup();
    void el.classList;

    el.classList = "a b";

    expect(lists).toEqual([{ value: "a b" }]);
  });

  it("keeps the list when a string is assigned", () => {
    const { el } = setup();
    const before = el.classList;

    el.classList = "a";

    expect(el.classList).toBe(before);
  });

  it("gives a comment stand-in no className", () => {
    const { el } = setup({}, "comment");

    expect("className" in el).toBe(false);
  });

  it("gives a comment stand-in no classList", () => {
    const { el } = setup({}, "comment");

    expect("classList" in el).toBe(false);
  });

  it("gives each handle its own list", () => {
    const { el } = setup();
    const other = createIncaElement(2, "element", "div");

    expect(other.classList).not.toBe(el.classList);
  });

  it("asks the native once on the first read of a freed handle", () => {
    const { el, host } = setup();
    markDestroyed(el);

    void el.classList;

    expect(host.classList).toHaveBeenCalledTimes(1);
  });
});
