// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { keep } from "../../tests/keep.mts";
import { nativeStub } from "../../tests/setup.mts";
import { createIncaElement, rootElement } from "./handle.mts";
import { markFreed, markRemoved } from "./lifecycle.mts";

describe("createIncaElement selectors", () => {
  /** A fake host whose `matches` and `closest` answer `found`; the queries find nothing. */
  function installSelectors(found: number | null = null, matched = false) {
    const host = {
      rootNodeId: vi.fn<() => number>(() => 0),
      matches: vi.fn<(nodeId: number, selector: string) => boolean>(() => matched),
      closest: vi.fn<(nodeId: number, selector: string) => number | null>(() => found),
      checkSelector: vi.fn<(selector: string) => void>(),
      querySelector: vi.fn<(nodeId: number, selector: string) => number | null>(() => null),
      querySelectorAll: vi.fn<(nodeId: number, selector: string) => number[]>(() => []),
    };
    vi.stubGlobal("__inca_native__", nativeStub(host as never));
    return host;
  }

  /** Three elements with ids 1, 2 and 3. */
  function chain() {
    const [outer, inner, leaf] = [1, 2, 3].map((id) => keep(createIncaElement(id, "div")));
    return { outer: outer!, inner: inner!, leaf: leaf! };
  }

  it("calls matches with the node id and the selector", () => {
    const host = installSelectors(null, true);
    const { leaf } = chain();

    expect(leaf.matches(".a")).toBe(true);
    expect(host.matches).toHaveBeenCalledWith(3, ".a");
  });

  it.each(["matches", "closest", "querySelector", "querySelectorAll"] as const)(
    "%s converts a number to text",
    (method) => {
      const host = installSelectors();
      const { leaf } = chain();

      leaf[method](1 as unknown as string);

      expect(host[method]).toHaveBeenCalledWith(3, "1");
    },
  );

  it.each(["matches", "closest", "querySelector", "querySelectorAll"] as const)(
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

  it("closest is null when the matched ancestor has no handle", () => {
    installSelectors(77);

    expect(chain().leaf.closest("div")).toBeNull();
  });

  it("closest is null when nothing matches", () => {
    installSelectors(null);

    expect(chain().leaf.closest("p")).toBeNull();
  });

  it.each([
    ["matches", false],
    ["closest", null],
    ["querySelector", null],
  ] as const)("%s answers %s for a freed handle without a host call", (method, expected) => {
    const host = installSelectors(1, true);
    const { leaf } = chain();
    markFreed([leaf.__node_id]);

    expect(leaf[method]("div")).toBe(expected);
    expect(host[method]).not.toHaveBeenCalled();
  });

  it("querySelectorAll answers an empty list for a freed handle without a host call", () => {
    const host = installSelectors();
    const { leaf } = chain();
    markFreed([leaf.__node_id]);

    expect([leaf.querySelectorAll("div").length, host.querySelectorAll.mock.calls]).toEqual([
      0,
      [],
    ]);
  });

  it.each(["matches", "closest", "querySelector", "querySelectorAll"] as const)(
    "%s throws SyntaxError on a freed element",
    (method) => {
      const host = installSelectors();
      host.checkSelector.mockImplementation(() => {
        throw new DOMException("the selector is not valid", "SyntaxError");
      });
      const { leaf } = chain();
      markFreed([leaf.__node_id]);

      expect(() => leaf[method]("div >")).toThrow(expect.objectContaining({ name: "SyntaxError" }));
    },
  );

  it.each(["matches", "closest", "querySelector", "querySelectorAll"] as const)(
    "%s throws TypeError for a missing argument on a freed element",
    (method) => {
      const host = installSelectors();
      const { leaf } = chain();
      markFreed([leaf.__node_id]);

      expect(() => (leaf[method] as () => unknown)()).toThrow(TypeError);
      expect(host.checkSelector).not.toHaveBeenCalled();
    },
  );

  it.each(["matches", "closest", "querySelector", "querySelectorAll"] as const)(
    "%s throws TypeError for a Symbol on a freed element",
    (method) => {
      const host = installSelectors();
      const { leaf } = chain();
      markFreed([leaf.__node_id]);

      expect(() => leaf[method](Symbol("s") as unknown as string)).toThrow(TypeError);
      expect(host.checkSelector).not.toHaveBeenCalled();
    },
  );

  it.each(["matches", "closest", "querySelector", "querySelectorAll"] as const)(
    "%s sends the converted selector text to checkSelector",
    (method) => {
      const host = installSelectors();
      const { leaf } = chain();
      markFreed([leaf.__node_id]);

      leaf[method](1 as unknown as string);

      expect(host.checkSelector).toHaveBeenCalledWith("1");
    },
  );

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

  it("querySelector returns null for a match with no handle", () => {
    const host = installSelectors();
    const { outer } = chain();
    host.querySelector.mockReturnValueOnce(77);

    expect(outer.querySelector("div")).toBeNull();
  });

  it("querySelectorAll skips a match with no handle", () => {
    const host = installSelectors();
    const { outer, inner } = chain();
    host.querySelectorAll.mockReturnValueOnce([77, 2]);

    expect(Array.from(outer.querySelectorAll("div")).map((node) => node.__node_id)).toEqual(
      [inner].map((node) => node.__node_id),
    );
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

  it("still asks the host for a removed handle", () => {
    const host = installSelectors(null, true);
    const { leaf } = chain();
    markRemoved([leaf.__node_id]);

    expect(leaf.matches("div")).toBe(true);
    expect(host.matches).toHaveBeenCalledTimes(1);
  });

  it.each(["matches", "closest", "querySelector", "querySelectorAll"] as const)(
    "%s throws a TypeError for no argument",
    (method) => {
      installSelectors();
      const { leaf } = chain();

      expect(() => (leaf[method] as () => unknown)()).toThrow(TypeError);
    },
  );
});
