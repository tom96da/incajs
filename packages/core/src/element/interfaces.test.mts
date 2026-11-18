// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { adoptComment, adoptElement, adoptFragment, adoptText } from "../../tests/adopt.mts";
import { installFakeHost } from "../../tests/fakeHost.mts";
import { collectGarbage } from "../../tests/gc.mts";
import { document } from "../document.mts";
import { window } from "../window.mts";
import { HANDLER_EVENTS } from "./eventHandlers.mts";
import { rootElement } from "./handle.mts";
import { defineMembers } from "./interfaces.mts";
import { prototypes } from "./lifecycle.mts";
import { insert, remove } from "./treeOps.mts";
import type { ElementName } from "../ffi/index.mts";

const otherName: ElementName = ["a", "urn:x", null, "a"];

const handlers = HANDLER_EVENTS.map((event) => `on${event}`);
const parentNode = [
  "children",
  "firstElementChild",
  "lastElementChild",
  "querySelector",
  "querySelectorAll",
];
const parentInsertion = ["append", "prepend", "replaceChildren"];
const childInsertion = ["before", "after", "replaceWith", "remove"];
const siblings = ["previousElementSibling", "nextElementSibling"];
const shared = ["style", "dataset", ...handlers];

/** The members the host defines. The stand-in prototypes of the tests hold some of them, and the Rust tests of the bridge pin the real set. */
const HOST_MEMBERS = new Set([
  "nodeType",
  "nodeName",
  "nodeValue",
  "isConnected",
  "hasChildNodes",
  "data",
  "length",
  "substringData",
  "appendData",
  "insertData",
  "deleteData",
  "replaceData",
  "wholeText",
  "id",
  "className",
  "tagName",
  "localName",
  "namespaceURI",
  "prefix",
  "getAttribute",
  "setAttribute",
  "hasAttribute",
  "removeAttribute",
  "getAttributeNames",
  "matches",
  "innerHTML",
  "outerHTML",
  "getBoundingClientRect",
  "clientWidth",
  "clientHeight",
  "clientLeft",
  "clientTop",
  "scrollLeft",
  "scrollTop",
  "scrollWidth",
  "scrollHeight",
  "scrollTo",
  "scrollBy",
  "scrollIntoView",
  "setPointerCapture",
  "releasePointerCapture",
  "hasPointerCapture",
  "click",
  "offsetWidth",
  "offsetHeight",
  "offsetLeft",
  "offsetTop",
  "focus",
  "blur",
  "tabIndex",
  "childElementCount",
]);

const MEMBERS: Record<string, string[]> = {
  EventTarget: ["addEventListener", "removeEventListener", "dispatchEvent"],
  Node: [
    "textContent",
    "ownerDocument",
    "parentNode",
    "parentElement",
    "firstChild",
    "lastChild",
    "previousSibling",
    "nextSibling",
    "childNodes",
    "appendChild",
    "insertBefore",
    "removeChild",
    "replaceChild",
    "cloneNode",
    "contains",
    "getRootNode",
    "compareDocumentPosition",
    "isSameNode",
    "isEqualNode",
  ],
  CharacterData: [...childInsertion, ...siblings],
  Text: ["splitText", "kind"],
  Comment: ["kind"],
  Element: [
    "classList",
    "attributes",
    "closest",
    "insertAdjacentHTML",
    ...parentNode,
    ...parentInsertion,
    ...childInsertion,
    ...siblings,
    "kind",
  ],
  HTMLElement: ["offsetParent", ...shared],
  ForeignElement: shared,
  DocumentFragment: [...parentNode, ...parentInsertion, "kind"],
  Document: [
    "activeElement",
    "createDocumentFragment",
    "createElement",
    "createElementNS",
    "createTextNode",
    "createComment",
    ...parentNode,
    ...parentInsertion,
    ...handlers,
    "kind",
  ],
  HTMLDocument: [],
};

/** The descriptor of a member of a prototype. */
function member(proto: object, name: string): PropertyDescriptor {
  return Object.getOwnPropertyDescriptor(proto, name)!;
}

/** Calls a method of a prototype on `self`. */
function call(proto: object, name: string, self: unknown, ...args: unknown[]): unknown {
  return Reflect.apply(member(proto, name).value as () => unknown, self, args);
}

/** Reads an accessor of a prototype on `self`. */
function read(proto: object, name: string, self: unknown): unknown {
  return member(proto, name).get!.call(self);
}

describe("the members of each prototype", () => {
  it.each(Object.entries(MEMBERS))("%s holds the members of the standard", (name, expected) => {
    const proto = (prototypes() as unknown as Record<string, object>)[name]!;

    const own = Object.getOwnPropertyNames(proto).filter((key) => !HOST_MEMBERS.has(key));

    expect(own.sort()).toEqual([...expected].sort());
  });

  it("gives window the handler properties as own members", () => {
    expect(Object.getOwnPropertyNames(window).sort()).toEqual([...handlers].sort());
  });

  it.each([
    ["an element", () => adoptElement(21), ["__node_id"]],
    ["a text node", () => adoptText(22, "x"), ["__node_id"]],
    ["an HTML input", () => adoptElement(23, "input"), ["__node_id", "value"]],
    ["an HTML template", () => document.createElement("template"), ["__node_id", "content"]],
  ] as const)("gives %s only its own state as keys", (_kind, make, keys) => {
    installFakeHost();

    expect(new Set(Reflect.ownKeys(make()))).toEqual(new Set(keys));
  });
});

describe("the data property kind", () => {
  it.each([
    ["Element", "element"],
    ["Text", "text"],
    ["Comment", "comment"],
    ["DocumentFragment", "fragment"],
    ["Document", "document"],
  ])("is a non-writable, non-enumerable, configurable value on %s", (name, kind) => {
    const proto = (prototypes() as unknown as Record<string, object>)[name]!;

    expect(member(proto, "kind")).toEqual({
      value: kind,
      writable: false,
      enumerable: false,
      configurable: true,
    });
  });
});

describe("the length of a method", () => {
  it.each([
    ["EventTarget", "addEventListener", 2],
    ["EventTarget", "removeEventListener", 2],
    ["EventTarget", "dispatchEvent", 1],
    ["Node", "appendChild", 1],
    ["Node", "insertBefore", 2],
    ["Node", "removeChild", 1],
    ["Node", "replaceChild", 2],
    ["Node", "cloneNode", 0],
    ["Node", "contains", 1],
    ["Node", "getRootNode", 0],
    ["Node", "compareDocumentPosition", 1],
    ["Node", "isSameNode", 1],
    ["Node", "isEqualNode", 1],
    ["Text", "splitText", 1],
    ["Element", "querySelector", 1],
    ["Element", "querySelectorAll", 1],
    ["DocumentFragment", "querySelector", 1],
    ["Document", "querySelectorAll", 1],
    ["Element", "closest", 1],
    ["Element", "insertAdjacentHTML", 2],
    ["Document", "createElement", 1],
    ["Document", "createElementNS", 2],
    ["Document", "createTextNode", 1],
    ["Document", "createComment", 1],
    ["Document", "createDocumentFragment", 0],
  ])("is the standard count for %s.%s", (name, method, length) => {
    const proto = (prototypes() as unknown as Record<string, object>)[name]!;

    expect((member(proto, method).value as () => void).length).toBe(length);
  });
});

describe("the members of a mixin", () => {
  it("are separate functions for each interface", () => {
    const all = prototypes();

    expect(member(all.Element, "querySelector").value).not.toBe(
      member(all.Document, "querySelector").value,
    );
  });

  it("have a handler getter of their own on each interface", () => {
    const all = prototypes();
    const getters = [all.HTMLElement, all.ForeignElement, all.Document, window].map(
      (owner) => (member(owner, "onclick") as { get: object }).get,
    );

    expect(new Set(getters).size).toBe(4);
  });
});

describe("the receiver check", () => {
  const all = prototypes();
  const text = () => adoptText(31, "x");

  it.each([
    ["a Node member on a plain object", () => call(all.Node, "appendChild", {}, {}), "a Node"],
    [
      "an EventTarget member on a plain object",
      () => call(all.EventTarget, "dispatchEvent", {}, {}),
      "an EventTarget",
    ],
    [
      "an Element member on a text node",
      () => call(all.Element, "closest", text(), "a"),
      "an Element",
    ],
    ["splitText on a comment", () => call(all.Text, "splitText", adoptComment(34), 0), "a Text"],
    [
      "previousElementSibling on document",
      () => read(all.CharacterData, "previousElementSibling", document),
      "an Element or CharacterData",
    ],
    [
      "previousElementSibling on a fragment",
      () => read(all.CharacterData, "previousElementSibling", adoptFragment(36)),
      "an Element or CharacterData",
    ],
    [
      "createElement on an element",
      () => call(all.Document, "createElement", adoptElement(37), "a"),
      "a Document",
    ],
    [
      "a ParentNode member on a text node",
      () => read(all.Element, "children", text()),
      "an Element, Document or DocumentFragment",
    ],
    [
      "a getter read on the prototype itself",
      () => read(all.Node, "ownerDocument", all.Node),
      "a Node",
    ],
    [
      "a member called on a forged object",
      () => call(all.Node, "appendChild", Object.create(all.HTMLElement), {}),
      "a Node",
    ],
    [
      "a member called on an object that inherits a handle",
      () => call(all.Node, "appendChild", Object.create(adoptElement(38)), {}),
      "a Node",
    ],
  ])("throws for %s", (_row, run, name) => {
    installFakeHost();

    expect(run).toThrow(
      new TypeError(`the receiver is not ${name}. call the method on an instance`),
    );
  });

  it("runs the children getter of Document.prototype on document", () => {
    installFakeHost();

    expect(() => read(all.Document, "children", document)).not.toThrow();
  });

  it("runs the children getter of DocumentFragment.prototype on a fragment", () => {
    installFakeHost();

    expect(() => read(all.DocumentFragment, "children", adoptFragment(43))).not.toThrow();
  });

  it("runs an Element member on an element of another namespace", () => {
    expect(read(all.Element, "tagName", adoptElement(45, otherName))).toBe("a");
  });

  it("rejects window as the node argument of an insertion", () => {
    installFakeHost();

    expect(() => document.appendChild(window as never)).toThrow(
      new TypeError(
        "parameter 1 is not a node. pass an element, a text node, a comment or a fragment",
      ),
    );
  });

  it("answers false from contains for document", () => {
    installFakeHost({ compareNodePosition: () => 0 });

    expect(adoptElement(44).contains(document)).toBe(false);
  });
});

describe("the receiver check before the argument count", () => {
  const all = prototypes();

  it.each([
    ["a creation member of Document", () => call(all.Document, "createElement", {}), "a Document"],
    ["a selector member of Element", () => call(all.Element, "closest", {}), "an Element"],
    ["a markup member of Element", () => call(all.Element, "insertAdjacentHTML", {}), "an Element"],
    ["a mutation member of Node", () => call(all.Node, "appendChild", {}), "a Node"],
    ["a relation member of Node", () => call(all.Node, "compareDocumentPosition", {}), "a Node"],
    ["splitText of Text", () => call(all.Text, "splitText", {}), "a Text"],
  ])("throws the receiver error for %s called with no arguments", (_row, run, name) => {
    expect(run).toThrow(
      new TypeError(`the receiver is not ${name}. call the method on an instance`),
    );
  });
});

describe("two handles", () => {
  it.each(["classList", "style", "dataset", "attributes", "childNodes", "children"])(
    "keep a separate %s",
    (name) => {
      installFakeHost({
        classList: () => ({}) as never,
        styleOf: () => ({}) as never,
        datasetTarget: () => ({}),
        attributeKey: (_id: number, key: string) => key,
      });
      const first = document.createElement("div") as unknown as Record<string, unknown>;
      const second = document.createElement("div") as unknown as Record<string, unknown>;

      expect(Object.is(first[name], second[name])).toBe(false);
    },
  );

  it("keep a separate handler property", () => {
    installFakeHost({ addEventListener: () => 1 });
    const first = document.createElement("div") as unknown as { onclick: unknown };
    const second = document.createElement("div") as unknown as { onclick: unknown };

    first.onclick = (): void => {};

    expect(second.onclick).toBeNull();
  });
});

describe("the collection of a handle whose cached members were read", () => {
  /** Makes a held element, runs `use` on it and returns its node id. */
  function made(use: (element: ReturnType<typeof document.createElement>) => void): number {
    const element = document.createElement("div");
    use(element);
    return element.__node_id;
  }

  /** Installs the fake host with the natives the members read. */
  function fakeHost() {
    return installFakeHost({ attributeKey: (_id: number, name: string) => name });
  }

  it.each([
    [
      "its attributes were read",
      (element: ReturnType<typeof document.createElement>) => {
        element.setAttribute("a", "1");
        element.attributes.getNamedItem("a");
      },
    ],
    [
      "its children were read",
      (element: ReturnType<typeof document.createElement>) => element.children.length,
    ],
    [
      "its childNodes were read",
      (element: ReturnType<typeof document.createElement>) => element.childNodes.length,
    ],
    [
      "its onclick returns the element and it is removed",
      (element: ReturnType<typeof document.createElement>) => {
        (element as unknown as { onclick: unknown }).onclick = () => element;
        insert(element, rootElement());
        remove(element);
      },
    ],
  ])("lets the host free the node of an element once %s", async (_name, use) => {
    const fake = fakeHost();
    const id = made(use);

    await collectGarbage(() => !fake.held.has(id));
    fake.frame();

    expect(fake.nodes.has(id)).toBe(false);
  });

  it("lets the host free a template and its content once the content was read", async () => {
    const fake = fakeHost();
    const ids = (() => {
      const template = document.createElement("template");
      void (template as unknown as { content: object }).content;
      return [template.__node_id, fake.nodes.get(template.__node_id)!.content!];
    })();

    await collectGarbage(() => ids.every((id) => !fake.held.has(id)));
    fake.frame();

    expect(ids.map((id) => fake.nodes.has(id))).toEqual([false, false]);
  });
});

describe("a second evaluation of core", () => {
  it("does not throw", async () => {
    vi.resetModules();

    await expect(import("./handle.mts")).resolves.toBeDefined();
  });

  it("builds a handle that answers nodeName", async () => {
    vi.resetModules();
    const again = await import("./handle.mts");

    expect((again.adoptHandle(61) as Element).nodeName).toBe("DIV");
  });

  it("throws the receiver error for a handle of the first evaluation", async () => {
    const before = adoptElement(62);
    vi.resetModules();
    await import("./handle.mts");

    expect(() => before.parentNode).toThrow(
      new TypeError("the receiver is not a Node. call the method on an instance"),
    );
  });
});

describe("defineMembers", () => {
  /** A prototype with the host getter of `value`, and the members `set` writes to `written`. */
  function setup() {
    const written: unknown[] = [];
    const proto = {};
    const hostGetter = () => "host";
    Object.defineProperty(proto, "value", {
      get: hostGetter,
      enumerable: true,
      configurable: true,
    });
    const members = {
      set value(next: unknown) {
        written.push(next);
      },
    };
    return { proto, hostGetter, members, written };
  }

  it("keeps the getter of the prototype beside a setter", () => {
    const { proto, hostGetter, members } = setup();

    defineMembers(proto, members);

    expect(Object.getOwnPropertyDescriptor(proto, "value")).toMatchObject({
      get: hostGetter,
      enumerable: true,
      configurable: true,
    });
  });

  it("calls the setter of the members", () => {
    const { proto, members, written } = setup();
    defineMembers(proto, members);

    (proto as { value: unknown }).value = 5;

    expect(written).toEqual([5]);
  });

  it("keeps the getter after a second evaluation", () => {
    const { proto, hostGetter, members } = setup();
    defineMembers(proto, members);
    defineMembers(proto, {
      set value(_next: unknown) {},
    });

    // oxlint-disable-next-line typescript/unbound-method
    expect(Object.getOwnPropertyDescriptor(proto, "value")!.get).toBe(hostGetter);
  });

  it("leaves a member with its own getter alone", () => {
    const { proto } = setup();

    defineMembers(proto, {
      get value() {
        return "own";
      },
      set value(_next: unknown) {},
    });

    expect((proto as { value: unknown }).value).toBe("own");
  });
});
