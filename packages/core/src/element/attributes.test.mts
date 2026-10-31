// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { createIncaElement } from "./handle.mts";
import { markDestroyed, markRemoved } from "./lifecycle.mts";
import type { IncaElement } from "./types.mts";

type Stored = string | number | boolean;

/** Installs a fake host that keeps attributes in a map, and returns it with a handle. */
function setup(initial: Record<string, Stored> = {}) {
  const store = new Map<string, Stored>(Object.entries(initial));
  const host = {
    getAttribute: vi.fn<(nodeId: number, key: string) => Stored | null>(
      (_id, key) => store.get(key.toLowerCase()) ?? null,
    ),
    setAttribute: vi.fn<(nodeId: number, key: string, value: Stored) => void>((_id, key, value) => {
      store.set(key.toLowerCase(), value);
    }),
    removeAttribute: vi.fn<(nodeId: number, key: string) => void>((_id, key) => {
      store.delete(key.toLowerCase());
    }),
    attributeNames: vi.fn<(nodeId: number) => string[]>(() => [...store.keys()].toSorted()),
  };
  globalThis.__inca_native__ = host as never;
  const el: IncaElement = createIncaElement(1, "element", "div");
  return { host, el, store };
}

describe("getAttribute", () => {
  it("returns a number as a string", () => {
    expect(setup({ tabindex: 2 }).el.getAttribute("tabindex")).toBe("2");
  });

  it("returns a boolean as a string", () => {
    expect(setup({ flag: true }).el.getAttribute("flag")).toBe("true");
  });

  it("returns null for an unset name", () => {
    expect(setup().el.getAttribute("missing")).toBeNull();
  });

  it("returns an empty string for an empty value", () => {
    expect(setup({ hidden: "" }).el.getAttribute("hidden")).toBe("");
  });

  it.each(["getAttribute", "hasAttribute", "removeAttribute"] as const)(
    "%s throws a TypeError without an argument",
    (method) => {
      const { el } = setup();

      expect(() => (el[method] as () => unknown)()).toThrow(TypeError);
    },
  );
});

describe("setAttribute", () => {
  it.each<[string, unknown, string]>([
    ["an object", { a: 1 }, "[object Object]"],
    ["false", false, "false"],
    ["a number", 3, "3"],
  ])("stores %s as its string form", (_name, value, expected) => {
    const { host, el } = setup();

    el.setAttribute("a", value as string);

    expect(host.setAttribute).toHaveBeenCalledExactlyOnceWith(1, "a", expected);
  });

  it("throws a TypeError for a Symbol value", () => {
    const { host, el } = setup();

    expect(() => el.setAttribute("a", Symbol("s") as unknown as string)).toThrow(TypeError);
    expect(host.setAttribute).not.toHaveBeenCalled();
  });

  it.each([[""], ["a b"], ["a/b"], ["a=b"], ["a>b"], ["a\0b"]])(
    "throws InvalidCharacterError for the name %j",
    (name) => {
      const { host, el } = setup();

      expect(() => el.setAttribute(name, "v")).toThrow(
        expect.objectContaining({ name: "InvalidCharacterError" }),
      );
      expect(host.setAttribute).not.toHaveBeenCalled();
    },
  );

  it.each([["a:b"], ["é"], ["data-x"], ["1a"], ["×"]])("accepts the name %j", (name) => {
    const { host, el } = setup();

    el.setAttribute(name, "v");

    expect(host.setAttribute).toHaveBeenCalledExactlyOnceWith(1, name, "v");
  });

  it("throws a TypeError for too few arguments", () => {
    const { host, el } = setup();

    expect(() => (el.setAttribute as (name: string) => void)("a")).toThrow(TypeError);
    expect(host.setAttribute).not.toHaveBeenCalled();
  });
});

describe("hasAttribute", () => {
  it("is true for an empty value", () => {
    expect(setup({ hidden: "" }).el.hasAttribute("hidden")).toBe(true);
  });

  it("is false for an unset name", () => {
    expect(setup().el.hasAttribute("hidden")).toBe(false);
  });
});

describe("removeAttribute", () => {
  it("removes a set attribute", () => {
    const { el } = setup({ a: "1" });

    el.removeAttribute("a");

    expect(el.hasAttribute("a")).toBe(false);
  });

  it("ignores an unset name", () => {
    const { el } = setup();

    expect(() => el.removeAttribute("missing")).not.toThrow();
  });
});

describe("attributes", () => {
  it("maps the names of the host to name and value objects in its order", () => {
    const { el } = setup({ id: "x", a: 1 });

    expect(el.attributes).toEqual([
      { name: "a", value: "1" },
      { name: "id", value: "x" },
    ]);
  });

  it("lists the names of the host in its order", () => {
    expect(setup({ id: "x", a: 1 }).el.getAttributeNames()).toEqual(["a", "id"]);
  });

  it("is a snapshot", () => {
    const { el } = setup({ a: "1" });
    const before = el.attributes;

    el.setAttribute("b", "2");

    expect(before).toEqual([{ name: "a", value: "1" }]);
  });
});

describe("a freed node", () => {
  it("reads no attribute", () => {
    const { el } = setup({ a: "1" });
    markDestroyed(el);

    expect([el.getAttribute("a"), el.hasAttribute("a")]).toEqual([null, false]);
  });

  it("lists no attribute", () => {
    const { el } = setup({ a: "1" });
    markDestroyed(el);

    expect([el.attributes, el.getAttributeNames()]).toEqual([[], []]);
  });

  it("ignores setAttribute", () => {
    const { host, el } = setup();
    markDestroyed(el);

    el.setAttribute("b", "2");

    expect(host.setAttribute).not.toHaveBeenCalled();
  });

  it("ignores removeAttribute", () => {
    const { host, el } = setup({ a: "1" });
    markDestroyed(el);

    el.removeAttribute("a");

    expect(host.removeAttribute).not.toHaveBeenCalled();
  });
});

describe("a removed node", () => {
  /** A removed node over a host that holds `a`. */
  function removed() {
    const { el } = setup({ a: "1" });
    markRemoved(el);
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
    expect(removed().attributes).toEqual([{ name: "a", value: "1" }]);
  });
});
