// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { installFakeHost } from "../../tests/fakeHost.mts";
import { collectGarbage } from "../../tests/gc.mts";
import { nativeStub } from "../../tests/setup.mts";
import { document } from "../document.mts";
import { HTML_NAMESPACE } from "../ffi/index.mts";
import { window } from "../window.mts";
import { HANDLER_EVENTS } from "./eventHandlers.mts";
import {
  createIncaComment,
  createIncaElement,
  createIncaFragment,
  rootElement,
} from "./handle.mts";
import { prototypes } from "./lifecycle.mts";
import { createIncaText } from "./text.mts";
import { insert, remove } from "./treeOps.mts";
import type { ElementName } from "../ffi/index.mts";

const SVG = "http://www.w3.org/2000/svg";
const svgName: ElementName = ["g", SVG, null, "g"];
const plainName: ElementName = ["x", null, null, "x"];
const otherName: ElementName = ["a", "urn:x", null, "a"];

const handlers = HANDLER_EVENTS.map((event) => `on${event}`);
const parentNode = [
  "children",
  "firstElementChild",
  "lastElementChild",
  "childElementCount",
  "querySelector",
  "querySelectorAll",
];
const parentInsertion = ["append", "prepend", "replaceChildren"];
const childInsertion = ["before", "after", "replaceWith", "remove"];
const siblings = ["previousElementSibling", "nextElementSibling"];
const shared = ["focus", "blur", "tabIndex", "style", "dataset", ...handlers];

const MEMBERS: Record<string, string[]> = {
  EventTarget: ["addEventListener", "removeEventListener", "dispatchEvent"],
  Node: [
    "nodeType",
    "nodeName",
    "nodeValue",
    "textContent",
    "isConnected",
    "ownerDocument",
    "parentNode",
    "parentElement",
    "firstChild",
    "lastChild",
    "previousSibling",
    "nextSibling",
    "childNodes",
    "hasChildNodes",
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
  CharacterData: [
    "data",
    "length",
    "substringData",
    "appendData",
    "insertData",
    "deleteData",
    "replaceData",
    ...childInsertion,
    ...siblings,
  ],
  Text: ["wholeText", "splitText", "kind"],
  Comment: ["kind"],
  Element: [
    "id",
    "className",
    "classList",
    "tagName",
    "localName",
    "namespaceURI",
    "prefix",
    "getAttribute",
    "setAttribute",
    "hasAttribute",
    "removeAttribute",
    "getAttributeNames",
    "attributes",
    "matches",
    "closest",
    "innerHTML",
    "outerHTML",
    "insertAdjacentHTML",
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
    ...parentNode,
    ...parentInsertion,
    ...childInsertion,
    ...siblings,
    "kind",
  ],
  HTMLElement: [
    "click",
    "offsetWidth",
    "offsetHeight",
    "offsetLeft",
    "offsetTop",
    "offsetParent",
    ...shared,
  ],
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

    expect(Object.getOwnPropertyNames(proto).sort()).toEqual([...expected].sort());
  });

  it("gives window the handler properties as own members", () => {
    expect(Object.getOwnPropertyNames(window).sort()).toEqual([...handlers].sort());
  });

  it.each([
    ["an element", () => createIncaElement(21), ["__node_id"]],
    ["a text node", () => createIncaText(22, "x"), ["__node_id", "text"]],
    ["an HTML input", () => createIncaElement(23, "input"), ["__node_id", "value"]],
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
    ["Node", "hasChildNodes", 0],
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
    ["CharacterData", "substringData", 2],
    ["CharacterData", "appendData", 1],
    ["CharacterData", "insertData", 2],
    ["CharacterData", "deleteData", 2],
    ["CharacterData", "replaceData", 3],
    ["Text", "splitText", 1],
    ["Element", "querySelector", 1],
    ["Element", "querySelectorAll", 1],
    ["DocumentFragment", "querySelector", 1],
    ["Document", "querySelectorAll", 1],
    ["Element", "getAttribute", 1],
    ["Element", "setAttribute", 2],
    ["Element", "hasAttribute", 1],
    ["Element", "removeAttribute", 1],
    ["Element", "getAttributeNames", 0],
    ["Element", "matches", 1],
    ["Element", "closest", 1],
    ["Element", "insertAdjacentHTML", 2],
    ["Element", "getBoundingClientRect", 0],
    ["Element", "scrollTo", 0],
    ["Element", "scrollBy", 0],
    ["Element", "scrollIntoView", 0],
    ["Element", "setPointerCapture", 1],
    ["Element", "releasePointerCapture", 1],
    ["Element", "hasPointerCapture", 1],
    ["HTMLElement", "click", 0],
    ["HTMLElement", "focus", 0],
    ["HTMLElement", "blur", 0],
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
  const text = () => createIncaText(31, "x");
  const svg = () => createIncaElement(32, svgName);

  it.each([
    ["a Node member on a plain object", () => call(all.Node, "hasChildNodes", {}), "a Node"],
    [
      "an EventTarget member on a plain object",
      () => call(all.EventTarget, "dispatchEvent", {}, {}),
      "an EventTarget",
    ],
    [
      "an Element member on a text node",
      () => call(all.Element, "getAttribute", text(), "a"),
      "an Element",
    ],
    ["click on an SVG element", () => call(all.HTMLElement, "click", svg()), "an HTMLElement"],
    [
      "a CharacterData member on an element",
      () => call(all.CharacterData, "appendData", createIncaElement(33), "x"),
      "a CharacterData",
    ],
    [
      "splitText on a comment",
      () => call(all.Text, "splitText", createIncaComment(34), 0),
      "a Text",
    ],
    [
      "focus on an element of no namespace",
      () => call(all.HTMLElement, "focus", createIncaElement(35, plainName)),
      "an HTML, SVG or MathML element",
    ],
    [
      "previousElementSibling on document",
      () => read(all.CharacterData, "previousElementSibling", document),
      "an Element or CharacterData",
    ],
    [
      "previousElementSibling on a fragment",
      () => read(all.CharacterData, "previousElementSibling", createIncaFragment(36)),
      "an Element or CharacterData",
    ],
    [
      "createElement on an element",
      () => call(all.Document, "createElement", createIncaElement(37), "a"),
      "a Document",
    ],
    [
      "a ParentNode member on a text node",
      () => read(all.Element, "children", text()),
      "an Element, Document or DocumentFragment",
    ],
    [
      "click on an element of another namespace",
      () => call(all.HTMLElement, "click", createIncaElement(39, otherName)),
      "an HTMLElement",
    ],
    [
      "focus on an element of another namespace",
      () => call(all.HTMLElement, "focus", createIncaElement(40, otherName)),
      "an HTML, SVG or MathML element",
    ],
    ["a getter read on the prototype itself", () => read(all.Node, "nodeType", all.Node), "a Node"],
    [
      "a member called on a forged object",
      () => call(all.Node, "hasChildNodes", Object.create(all.HTMLElement)),
      "a Node",
    ],
    [
      "a member called on an object that inherits a handle",
      () => call(all.Node, "hasChildNodes", Object.create(createIncaElement(38))),
      "a Node",
    ],
  ])("throws for %s", (_row, run, name) => {
    installFakeHost();

    expect(run).toThrow(
      new TypeError(`the receiver is not ${name}. call the method on an instance`),
    );
  });

  it("runs click on an element that createElementNS made in the HTML namespace", () => {
    const click = vi.fn<(id: number) => void>();
    installFakeHost({ click });
    const element = document.createElementNS(HTML_NAMESPACE, "div");

    call(all.HTMLElement, "click", element);

    expect(click).toHaveBeenCalledExactlyOnceWith(element.__node_id);
  });

  it.each([
    ["an SVG element", () => svg()],
    [
      "an element of an HTML namespace",
      () => createIncaElement(42, ["a", HTML_NAMESPACE, null, "A"]),
    ],
  ])("runs focus on %s", (_row, make) => {
    const focus = vi.fn<(id: number, preventScroll: boolean) => void>();
    globalThis.__inca_native__ = nativeStub({ focusNode: focus } as never);
    const element = make();

    call(all.HTMLElement, "focus", element);

    expect(focus).toHaveBeenCalledOnce();
  });

  it("runs the children getter of Document.prototype on document", () => {
    installFakeHost();

    expect(() => read(all.Document, "children", document)).not.toThrow();
  });

  it("runs the children getter of DocumentFragment.prototype on a fragment", () => {
    installFakeHost();

    expect(() => read(all.DocumentFragment, "children", createIncaFragment(43))).not.toThrow();
  });

  it("runs an Element member on an element of another namespace", () => {
    expect(read(all.Element, "tagName", createIncaElement(45, otherName))).toBe("a");
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

    expect(createIncaElement(44).contains(document)).toBe(false);
  });
});

describe("the receiver check before the argument count", () => {
  const all = prototypes();

  it.each([
    ["a creation member of Document", () => call(all.Document, "createElement", {}), "a Document"],
    ["an attribute member of Element", () => call(all.Element, "getAttribute", {}), "an Element"],
    ["a markup member of Element", () => call(all.Element, "insertAdjacentHTML", {}), "an Element"],
    ["a mutation member of Node", () => call(all.Node, "appendChild", {}), "a Node"],
    ["a relation member of Node", () => call(all.Node, "compareDocumentPosition", {}), "a Node"],
    [
      "a data member of CharacterData",
      () => call(all.CharacterData, "substringData", {}),
      "a CharacterData",
    ],
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

    expect(again.createIncaElement(61).nodeName).toBe("DIV");
  });

  it("throws the receiver error for a handle of the first evaluation", async () => {
    const before = createIncaElement(62);
    vi.resetModules();
    await import("./handle.mts");

    expect(() => before.nodeName).toThrow(
      new TypeError("the receiver is not a Node. call the method on an instance"),
    );
  });
});
