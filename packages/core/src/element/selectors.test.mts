// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { adoptElement } from "../../tests/adopt.mts";
import { keep } from "../../tests/keep.mts";
import { nativeStub } from "../../tests/setup.mts";
import { rootElement } from "./handle.mts";
import { markRemoved } from "./lifecycle.mts";

describe("element selectors", () => {
  /** A fake host whose `closest` answers `found`; the queries find nothing. */
  function installSelectors(found: number | null = null) {
    const host = {
      rootNodeId: vi.fn<() => number>(() => 0),
      closest: vi.fn<(nodeId: number, selector: string) => number | null>(() => found),
      querySelector: vi.fn<(nodeId: number, selector: string) => number | null>(() => null),
      querySelectorAll: vi.fn<(nodeId: number, selector: string) => number[]>(() => []),
    };
    vi.stubGlobal("__inca_native__", nativeStub(host as never));
    return host;
  }

  /** Three elements with ids 1, 2 and 3. */
  function chain() {
    const [outer, inner, leaf] = [1, 2, 3].map((id) => keep(adoptElement(id, "div")));
    return { outer: outer!, inner: inner!, leaf: leaf! };
  }

  it.each(["closest", "querySelector", "querySelectorAll"] as const)(
    "%s converts a number to text",
    (method) => {
      const host = installSelectors();
      const { leaf } = chain();

      leaf[method](1 as unknown as string);

      expect(host[method]).toHaveBeenCalledWith(3, "1");
    },
  );

  it.each(["closest", "querySelector", "querySelectorAll"] as const)(
    "%s throws a TypeError for a Symbol",
    (method) => {
      installSelectors();
      const { leaf } = chain();

      expect(() => leaf[method](Symbol("s") as unknown as string)).toThrow(TypeError);
    },
  );

  it("calls closest with the node id and resolves the id to the ancestor handle", () => {
    const host = installSelectors(1);
    const { outer, leaf } = chain();

    expect(leaf.closest("div")).toBe(outer);
    expect(host.closest).toHaveBeenCalledWith(3, "div");
  });

  it("closest resolves to the element itself when the host names it", () => {
    installSelectors(3);
    const { leaf } = chain();

    expect(leaf.closest("div")).toBe(leaf);
  });

  it("closest resolves the mount container before any handle of it exists", () => {
    const host = installSelectors(91);
    vi.stubGlobal("__inca_native__", nativeStub({ ...host, rootNodeId: () => 91 } as never));
    const { leaf } = chain();

    expect(leaf.closest("div")).toBe(rootElement());
  });

  it("closest builds the handle of a matched ancestor that has none", () => {
    installSelectors(77);

    expect(chain().leaf.closest("div")?.__node_id).toBe(77);
  });

  it("closest is null when nothing matches", () => {
    installSelectors(null);

    expect(chain().leaf.closest("p")).toBeNull();
  });

  it("querySelector returns the handle of the id", () => {
    const host = installSelectors();
    const { outer, inner } = chain();
    host.querySelector.mockReturnValueOnce(2);

    expect(outer.querySelector("div")).toBe(inner);
  });

  it("querySelector calls the host with the node id and the selector", () => {
    const host = installSelectors();
    const { outer } = chain();

    outer.querySelector("div");

    expect(host.querySelector).toHaveBeenCalledWith(1, "div");
  });

  it("querySelector builds the handle of a match that has none", () => {
    const host = installSelectors();
    const { outer } = chain();
    host.querySelector.mockReturnValueOnce(77);

    expect(outer.querySelector("div")?.__node_id).toBe(77);
  });

  it("querySelectorAll builds the handle of a match that has none", () => {
    const host = installSelectors();
    const { outer, inner } = chain();
    host.querySelectorAll.mockReturnValueOnce([77, 2]);

    expect(Array.from(outer.querySelectorAll("div")).map((node) => node.__node_id)).toEqual([
      77,
      inner.__node_id,
    ]);
  });

  it("querySelector returns null without a match", () => {
    installSelectors();

    expect(chain().leaf.querySelector("div")).toBeNull();
  });

  it("querySelectorAll returns the handles in native order", () => {
    const host = installSelectors();
    const { outer, inner, leaf } = chain();
    host.querySelectorAll.mockReturnValueOnce([3, 2]);

    const found = outer.querySelectorAll("div");

    expect(found[0]).toBe(leaf);
    expect(found[1]).toBe(inner);
    expect(host.querySelectorAll).toHaveBeenCalledWith(1, "div");
  });

  it("querySelectorAll returns a list that later answers do not reach", () => {
    const host = installSelectors();
    const { outer } = chain();
    host.querySelectorAll.mockReturnValueOnce([2]);

    const found = outer.querySelectorAll("div");
    host.querySelectorAll.mockReturnValue([2, 3]);

    expect(found.length).toBe(1);
  });

  it("querySelectorAll gives a list a loop visits in full after a shorter answer", () => {
    const host = installSelectors();
    const { outer } = chain();
    host.querySelectorAll.mockReturnValueOnce([2, 3]);
    const found = outer.querySelectorAll("div");
    host.querySelectorAll.mockReturnValue([]);
    const visited: unknown[] = [];

    for (const el of found) visited.push(el);

    expect(visited.length).toBe(2);
  });

  it("querySelectorAll returns a new list on every call", () => {
    installSelectors();
    const { outer } = chain();

    expect(outer.querySelectorAll("div")).not.toBe(outer.querySelectorAll("div"));
  });

  it.each(["querySelector", "querySelectorAll"] as const)(
    "%s still asks the host for a removed handle",
    (method) => {
      const host = installSelectors();
      const { leaf } = chain();
      markRemoved([leaf.__node_id]);

      leaf[method]("div");

      expect(host[method]).toHaveBeenCalledTimes(1);
    },
  );

  it.each(["closest", "querySelector", "querySelectorAll"] as const)(
    "%s throws a TypeError for no argument",
    (method) => {
      installSelectors();
      const { leaf } = chain();

      expect(() => (leaf[method] as () => unknown)()).toThrow(TypeError);
    },
  );
});
