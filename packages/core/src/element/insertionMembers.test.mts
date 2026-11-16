// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { installFakeHost } from "../../tests/fakeHost.mts";
import { collectGarbage, collectSentinel } from "../../tests/gc.mts";
import { document } from "../document.mts";
import {
  createCommentNode,
  createElementNode,
  createIncaFragment,
  createTextNode,
} from "./handle.mts";
import { markFreed, nodeOf, prototypes } from "./lifecycle.mts";
import type { FakeHost } from "../../tests/fakeHost.mts";
import type { IncaElement, IncaNode } from "./types.mts";

type Native = typeof globalThis.__inca_native__;

const insertNodes = vi.fn<Native["insertNodes"]>();
let host: FakeHost;

beforeEach(() => {
  vi.clearAllMocks();
  insertNodes.mockReturnValue(null);
  host = installFakeHost({ insertNodes });
});

const div = () => createElementNode("div");
const fragment = () => createIncaFragment(host.create("#fragment"));
/** The reply of the native: the new text ids and the removed ids. */
const reply = (texts: number[] = [], removed: number[] = []) =>
  insertNodes.mockReturnValue([texts, removed]);
/** Calls the member `name` of `target`. */
const call = (target: object, name: string, ...args: unknown[]): unknown =>
  Reflect.apply((target as Record<string, () => unknown>)[name]!, target, args);

const PARENT_MEMBERS = ["append", "prepend", "replaceChildren"];
const CHILD_MEMBERS = ["before", "after", "replaceWith", "remove"];
const RECEIVERS: Record<string, () => IncaNode> = {
  element: div,
  document: () => document,
  fragment,
  text: () => createTextNode("t"),
  comment: () => createCommentNode(),
};

describe("the call to the host", () => {
  it.each<[string, string]>([
    ...PARENT_MEMBERS.map((name): [string, string] => [name, "element"]),
    ...CHILD_MEMBERS.map((name): [string, string] => [name, "element"]),
    ["append", "document"],
    ["prepend", "fragment"],
    ["replaceChildren", "fragment"],
    ["before", "text"],
    ["remove", "text"],
    ["before", "comment"],
    ["after", "comment"],
    ["replaceWith", "comment"],
    ["remove", "comment"],
  ])("%s on %s passes its place, the target and the converted list", (name, kind) => {
    const target = RECEIVERS[kind]!();
    const child = div();
    reply([host.create("#text")]);

    if (name === "remove") call(target, name);
    else call(target, name, child, "s");

    expect(insertNodes).toHaveBeenCalledExactlyOnceWith(
      target.__node_id,
      name === "remove" ? "replaceWith" : name,
      name === "remove" ? [] : [child.__node_id, "s"],
    );
  });

  it.each([
    ["null", null, "null"],
    ["an object", { toString: () => "o" }, "o"],
  ])("passes %s as its string", (_name, value, text) => {
    reply([host.create("#text")]);

    div().append(value as never);

    expect(insertNodes.mock.calls[0]![2]).toEqual([text]);
  });

  const SYMBOL_TARGETS: [string, () => IncaElement][] = [
    ["a live target", () => div()],
    [
      "a freed target",
      () => {
        const target = div();
        markFreed([target.__node_id]);
        return target;
      },
    ],
  ];

  it.each(SYMBOL_TARGETS)(
    "throws a TypeError and calls no native for a Symbol on %s",
    (_name, make) => {
      const target = make();

      expect(() => target.before(Symbol("s") as never)).toThrow(
        new TypeError("a symbol cannot become text"),
      );
      expect(insertNodes).not.toHaveBeenCalled();
    },
  );

  it("gives a new text handle the id of the host and the string as data", () => {
    const id = host.create("#text");
    reply([id]);

    div().append("s");

    expect(nodeOf(id)).toMatchObject({ __node_id: id, text: "s" });
  });

  it.each([...PARENT_MEMBERS, ...CHILD_MEMBERS])("%s returns undefined", (name) => {
    reply();

    expect(call(div(), name)).toBeUndefined();
  });

  it.each(CHILD_MEMBERS)("%s on a freed node calls no native", (name) => {
    const target = div();
    markFreed([target.__node_id]);

    call(target, name, "s");

    expect(insertNodes).not.toHaveBeenCalled();
  });
});

describe("the node table", () => {
  it.each([
    [
      "a new text handle",
      () => {
        const id = host.create("#text");
        reply([id]);
        div().append("s");
        return id;
      },
    ],
    [
      "an inserted node",
      () => {
        const child = div();
        reply();
        div().append(child);
        return child.__node_id;
      },
    ],
  ])("keeps %s after a collection", async (_name, insert) => {
    const id = insert();

    await collectSentinel();

    expect(nodeOf(id)).toBeDefined();
  });

  it("lets a fragment argument be collected", async () => {
    const ref = (() => {
      const group = fragment();
      reply();
      div().append(group);
      return new WeakRef(group);
    })();

    await collectGarbage(() => ref.deref() === undefined);

    expect(ref.deref()).toBeUndefined();
  });

  it.each([
    [
      "replaceChildren",
      () => div(),
      (_child: IncaNode, parent: IncaNode) => call(parent, "replaceChildren"),
    ],
    ["replaceWith", () => div(), (child: IncaNode) => call(child, "replaceWith")],
    ["remove", () => div(), (child: IncaNode) => call(child, "remove")],
    ["remove on a comment", () => createCommentNode(), (child: IncaNode) => call(child, "remove")],
  ])("%s holds the removed node and frees its handle", async (_name, make, run) => {
    const parent = div();
    const [ref, id] = (() => {
      const child = make();
      parent.appendChild(child);
      reply([], [child.__node_id]);
      run(child, parent);
      return [new WeakRef(child), child.__node_id] as const;
    })();

    await collectGarbage(() => ref.deref() === undefined);

    expect([ref.deref(), host.spies.holdNode.mock.calls]).toEqual([undefined, [[id]]]);
  });

  it.each([
    ["an id nobody has a handle for", () => host.create("div")],
    ["the collected mount container", () => host.ROOT],
  ])("skips %s among the removed nodes", (_name, id) => {
    reply([], [id()]);

    div().remove();

    expect([nodeOf(host.ROOT), host.spies.holdNode.mock.calls]).toEqual([undefined, []]);
  });

  it.each([
    [
      "before",
      () => {
        const child = div();
        div().before(child);
        return new WeakRef(child);
      },
    ],
    [
      "remove",
      () => {
        const target = div();
        target.remove();
        return new WeakRef(target);
      },
    ],
  ])("%s with a null result enters and holds nothing", async (_name, run) => {
    const ref = run();

    await collectGarbage(() => ref.deref() === undefined);

    expect([ref.deref(), host.spies.holdNode.mock.calls]).toEqual([undefined, []]);
  });

  it("leaves a node argument collectable when the native throws", async () => {
    insertNodes.mockImplementation(() => {
      throw new DOMException("no", "HierarchyRequestError");
    });
    // The call runs directly: a closure passed to `expect` is kept by the assertion and holds `child`.
    const ref = (() => {
      const child = div();
      let thrown: unknown;
      try {
        div().append("s", child);
      } catch (error) {
        thrown = error;
      }
      expect(thrown).toBeInstanceOf(DOMException);
      return new WeakRef(child);
    })();

    await collectGarbage(() => ref.deref() === undefined);

    expect([ref.deref(), host.spies.holdNode.mock.calls]).toEqual([undefined, []]);
  });
});

describe("the members on the prototypes", () => {
  const all = prototypes() as unknown as Record<string, object>;
  const WHERE: [string, string[]][] = [
    ["Element", [...PARENT_MEMBERS, ...CHILD_MEMBERS]],
    ["Document", PARENT_MEMBERS],
    ["DocumentFragment", PARENT_MEMBERS],
    ["CharacterData", CHILD_MEMBERS],
  ];

  it.each(WHERE)("%s has its members", (name, members) => {
    expect(members.filter((member) => Object.hasOwn(all[name]!, member))).toEqual(members);
  });

  it.each([["Node"], ["Text"]])("%s has none of the members", (name) => {
    const found = [...PARENT_MEMBERS, ...CHILD_MEMBERS].filter((member) =>
      Object.hasOwn(all[name]!, member),
    );

    expect(found).toEqual([]);
  });

  it.each(WHERE.flatMap(([name, members]) => members.map((member) => [name, member])))(
    "%s.%s is a method of length 0 that can be written, listed and configured",
    (name, member) => {
      const descriptor = Object.getOwnPropertyDescriptor(all[name]!, member)!;

      expect([
        typeof descriptor.value,
        (descriptor.value as () => void).length,
        descriptor.writable,
        descriptor.enumerable,
        descriptor.configurable,
      ]).toEqual(["function", 0, true, true, true]);
    },
  );

  it.each([
    ["append", "Element", "Document"],
    ["append", "Element", "DocumentFragment"],
    ["append", "Document", "DocumentFragment"],
    ["before", "Element", "CharacterData"],
  ])("gives %s its own function on %s and %s", (member, first, second) => {
    expect(all[first]![member as never]).not.toBe(all[second]![member as never]);
  });

  it.each(WHERE)("lists the members of %s as unscopable", (name, members) => {
    const descriptor = Object.getOwnPropertyDescriptor(all[name]!, Symbol.unscopables)!;
    const list = descriptor.value as Record<string, unknown>;

    expect({
      names: new Set(Object.keys(list)),
      values: new Set(Object.values(list)),
      proto: Object.getPrototypeOf(list) as unknown,
      attributes: [descriptor.writable, descriptor.enumerable, descriptor.configurable],
    }).toEqual({
      names: new Set(members),
      values: new Set([true]),
      proto: null,
      attributes: [false, false, true],
    });
  });
});

describe("the receiver check", () => {
  it.each([
    ["a ParentNode member on a text node", "Element", "append", () => createTextNode("t")],
    ["a ChildNode member on a fragment", "CharacterData", "before", fragment],
    ["a ChildNode member on the document", "CharacterData", "remove", () => document],
  ])("%s throws before it converts an argument", (_name, owner, member, make) => {
    const calls: string[] = [];
    const watcher = { toString: () => (calls.push("toString"), "w") };
    const proto = (prototypes() as unknown as Record<string, Record<string, () => void>>)[owner]!;

    expect(() => Reflect.apply(proto[member]!, make(), [watcher])).toThrow(TypeError);
    expect([calls, insertNodes.mock.calls]).toEqual([[], []]);
  });
});
