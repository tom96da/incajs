// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { adoptElement } from "../../tests/adopt.mts";
import { nativeStub } from "../../tests/setup.mts";
import { markFreed } from "./lifecycle.mts";
import type { Element } from "./types.mts";

type Stored = string | number | boolean;

/** Installs a fake host with one attribute map per node, and returns it with a handle. */
function setup(initial: Record<string, Stored> = {}) {
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
  vi.stubGlobal("__inca_native__", nativeStub(host as never));
  const el: Element = adoptElement(1, "div");
  return { host, el, store, lists };
}

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

  it("gives each handle its own list", () => {
    const { el } = setup();
    const other = adoptElement(2, "div");

    expect(other.classList).not.toBe(el.classList);
  });

  it("asks the native once on the first read of a freed handle", () => {
    const { el, host } = setup();
    markFreed([el.__node_id]);

    void el.classList;

    expect(host.classList).toHaveBeenCalledTimes(1);
  });
});
