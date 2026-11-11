// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { nativeStub } from "../tests/setup.mts";
import { document } from "./document.mts";
import { HANDLER_EVENTS } from "./element/eventHandlers.mts";
import { createElementNode, rootElement } from "./element/handle.mts";
import { isHeld, markDestroyed, registerNode, unregisterNode } from "./element/lifecycle.mts";
import "./targets.mts";
import { HTML_NAMESPACE } from "./native.mts";
import type { IncaElement } from "./element/types.mts";
import type { ElementEntry, HostCallback, NodeLink } from "./native.mts";

const SVG_NAMESPACE = "http://www.w3.org/2000/svg";

/** The id `nativeStub` gives the document. */
const DOCUMENT = 1;

let nextCallbackId = 0;
let nextId = 100;
let focused: number | null = null;
/** The data of each text node or comment stand-in. */
let data: Record<number, unknown> = {};
/** The parent each node reaches through `linkedNode`. */
let parents = new Map<number, number>();
let found: number | null = null;
let foundAll: number[] = [];

const native = {
  focusedNode: vi.fn<() => number | null>(() => focused),
  addEventListener: vi.fn<
    (
      target: number | string,
      event: string,
      callback: HostCallback,
      capture?: boolean,
      passive?: boolean,
    ) => number
  >(() => nextCallbackId++),
  removeEventListener: vi.fn<
    (target: number | string, event: string, callbackId: number) => boolean
  >(() => true),
  dispatchEvent: vi.fn<(target: number | string, event: unknown) => boolean>(() => true),
  rootNodeId: vi.fn<() => number>(() => 0),
  createNode: vi.fn<(tag: string) => number>(() => nextId++),
  createElement: vi.fn<(localName: string) => [number, string]>((localName) => [
    nextId++,
    localName.toLowerCase(),
  ]),
  createElementNS: vi.fn<(namespace: string | null, qualifiedName: string) => ElementEntry>(
    (namespace, qualifiedName) => [nextId++, qualifiedName, namespace, null, qualifiedName],
  ),
  setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(
    (nodeId, _key, value) => {
      data[nodeId] = value;
    },
  ),
  getAttribute: vi.fn<(nodeId: number, key: string) => unknown>((nodeId) => data[nodeId] ?? null),
  setStyle: vi.fn<() => void>(),
  appendChild: vi.fn<(parentId: number, childId: number) => number>(() => 0),
  removeChild: vi.fn<(parentId: number, childId: number) => void>(),
  releaseListeners: vi.fn<(nodeId: number) => number[]>(() => []),
  holdNode: vi.fn<(nodeId: number) => void>(),
  releaseNode: vi.fn<(nodeId: number) => void>(),
  textContent: vi.fn<(nodeId: number) => string>(() => "joined"),
  isConnected: vi.fn<(nodeId: number) => boolean>(() => false),
  layoutOf: vi.fn<() => null>(() => null),
  linkedNode: vi.fn<(nodeId: number, link: NodeLink) => number | null>((nodeId, link) =>
    link === "parent" ? (parents.get(nodeId) ?? null) : null,
  ),
  querySelector: vi.fn<(nodeId: number, selector: string) => number | null>(() => found),
  querySelectorAll: vi.fn<(nodeId: number, selector: string) => number[]>(() => foundAll),
};

beforeEach(() => {
  vi.clearAllMocks();
  nextCallbackId = 0;
  nextId = 100;
  focused = null;
  data = {};
  parents = new Map();
  found = null;
  foundAll = [];
  globalThis.__inca_native__ = nativeStub(native as never);
});

afterEach(() => {
  unregisterNode(5);
});

describe("document", () => {
  it("is an instance of Document", () => {
    expect(document).toBeInstanceOf(Document);
  });

  it("reads the focused element as activeElement", () => {
    const handle = { __node_id: 5, kind: "element" } as unknown as IncaElement;
    registerNode(handle);
    const states = [];

    states.push(document.activeElement);
    focused = 5;
    states.push(document.activeElement);
    focused = null;
    states.push(document.activeElement);

    expect(states).toEqual([null, handle, null]);
  });

  it("reads null as activeElement for a focused id with no registered element", () => {
    focused = 999;

    expect(document.activeElement).toBeNull();
  });

  it("registers a listener on the document node", () => {
    document.addEventListener("keydown", vi.fn<() => void>());

    expect(native.addEventListener).toHaveBeenCalledExactlyOnceWith(
      DOCUMENT,
      "keydown",
      expect.any(Function),
      false,
      false,
    );
  });

  it("removes a listener from the document node", () => {
    const listener = vi.fn<() => void>();
    document.addEventListener("keydown", listener);
    const callbackId = native.addEventListener.mock.results[0]!.value;

    document.removeEventListener("keydown", listener);

    expect(native.removeEventListener).toHaveBeenCalledExactlyOnceWith(
      DOCUMENT,
      "keydown",
      callbackId,
    );
  });

  it("dispatches at the document node", () => {
    const event = { type: "ping" };

    document.dispatchEvent(event);

    expect(native.dispatchEvent).toHaveBeenCalledExactlyOnceWith(DOCUMENT, event);
  });

  it("makes wheel listeners passive unless the options say otherwise", () => {
    document.addEventListener("wheel", vi.fn<() => void>());
    document.addEventListener("wheel", vi.fn<() => void>(), { passive: false });
    document.addEventListener("scroll", vi.fn<() => void>());

    expect(native.addEventListener.mock.calls.map((call) => call[4])).toEqual([true, false, false]);
  });

  it("carries every HANDLER_EVENTS property", () => {
    const missing = HANDLER_EVENTS.filter((event) => !(`on${event}` in document));

    expect(missing).toEqual([]);
  });

  it("runs an onkeydown handler as a listener of the document", () => {
    const keys: unknown[] = [];

    document.onkeydown = function (this: unknown, event: { key: string }) {
      keys.push(this, event.key);
    };
    native.addEventListener.mock.calls[0]![2]({ key: "a", currentTarget: document });

    expect(native.addEventListener).toHaveBeenCalledExactlyOnceWith(
      DOCUMENT,
      "keydown",
      expect.any(Function),
      false,
      false,
    );
    expect(keys).toEqual([document, "a"]);
  });
});

describe("node members", () => {
  it("reads 9 as nodeType", () => {
    expect(document.nodeType).toBe(9);
  });

  it('reads "#document" as nodeName', () => {
    expect(document.nodeName).toBe("#document");
  });

  it("reads null as ownerDocument", () => {
    expect(document.ownerDocument).toBeNull();
  });

  it("reads null as textContent", () => {
    expect(document.textContent).toBeNull();
  });

  it("ignores an assignment to textContent", () => {
    document.textContent = "x";

    expect(native.setAttribute).not.toHaveBeenCalled();
    expect(native.createNode).not.toHaveBeenCalled();
  });

  it("reads null as nodeValue", () => {
    expect(document.nodeValue).toBeNull();
  });

  it("ignores an assignment to nodeValue", () => {
    document.nodeValue = "x";

    expect(native.setAttribute).not.toHaveBeenCalled();
  });

  it("has no cloneNode", () => {
    expect("cloneNode" in document).toBe(false);
  });

  it("links the mount container back after it was removed", () => {
    const root = rootElement();

    document.removeChild(root);
    document.appendChild(root);

    expect(native.removeChild).toHaveBeenCalledExactlyOnceWith(DOCUMENT, 0);
    expect(native.appendChild).toHaveBeenCalledExactlyOnceWith(DOCUMENT, 0);
    expect(root.parent).toBe(document);
    expect(document.__children).toEqual([root]);
  });
});

describe("createElement", () => {
  it("passes the name as given to the host", () => {
    document.createElement("DIV");

    expect(native.createElement).toHaveBeenCalledExactlyOnceWith("DIV");
  });

  it("reads the id and the tag the host answers", () => {
    native.createElement.mockImplementationOnce(() => [77, "span"]);

    const el = document.createElement("DIV");

    expect([el.__node_id, el.tagName, el.nodeName]).toEqual([77, "SPAN", "SPAN"]);
  });

  it("converts the name once", () => {
    let conversions = 0;
    const name = {
      toString: () => {
        conversions++;
        return "div";
      },
    };

    document.createElement(name as never);

    expect(conversions).toBe(1);
  });

  it("passes the error of an invalid name through", () => {
    const error = new DOMException("the element name is not valid", "InvalidCharacterError");
    native.createElement.mockImplementationOnce(() => {
      throw error;
    });

    expect(() => document.createElement("1a")).toThrow(error);
  });
});

describe("the name members of createElement", () => {
  it.each([
    ["svg", ["SVG", "svg"]],
    ["DIV", ["DIV", "div"]],
  ] as const)("reads %s as tagName and localName", (name, expected) => {
    const el = document.createElement(name);

    expect([el.tagName, el.localName, el.namespaceURI, el.prefix]).toEqual([
      ...expected,
      HTML_NAMESPACE,
      null,
    ]);
  });
});

describe("createElementNS", () => {
  it("reads the four members and nodeName the host answers", () => {
    native.createElementNS.mockImplementationOnce(() => [77, "item", SVG_NAMESPACE, "x", "x:item"]);

    const el = document.createElementNS(SVG_NAMESPACE, "x:item");

    expect([
      el.__node_id,
      el.tagName,
      el.nodeName,
      el.localName,
      el.namespaceURI,
      el.prefix,
    ]).toEqual([77, "x:item", "x:item", "item", SVG_NAMESPACE, "x"]);
  });

  it.each([
    ["null", null, null],
    ["undefined", undefined, null],
    ["an empty string", "", null],
    ["a number", 1, "1"],
    ["a string", SVG_NAMESPACE, SVG_NAMESPACE],
  ] as const)("passes %s as the namespace %j to the host", (_name, namespace, passed) => {
    document.createElementNS(namespace as never, "a");

    expect(native.createElementNS).toHaveBeenCalledExactlyOnceWith(passed, "a");
  });

  it("converts the name to a string", () => {
    document.createElementNS(null, undefined as never);

    expect(native.createElementNS).toHaveBeenCalledExactlyOnceWith(null, "undefined");
  });

  it("ignores a third argument", () => {
    (document.createElementNS as (...args: unknown[]) => unknown)(null, "a", "extra");

    expect(native.createElementNS).toHaveBeenCalledExactlyOnceWith(null, "a");
  });

  it.each([
    ["no argument", []],
    ["one argument", [SVG_NAMESPACE]],
  ] as const)("throws a TypeError for %s", (_name, args) => {
    expect(() => (document.createElementNS as (...a: unknown[]) => unknown)(...args)).toThrow(
      TypeError,
    );
  });

  it("passes the error of an invalid name through", () => {
    const error = new DOMException("the element name is not valid", "InvalidCharacterError");
    native.createElementNS.mockImplementationOnce(() => {
      throw error;
    });

    expect(() => document.createElementNS(null, "1a")).toThrow(error);
  });

  it("holds the element until its handle is collected", () => {
    const el = document.createElementNS(SVG_NAMESPACE, "svg");

    expect(isHeld(el)).toBe(true);
  });
});

describe("created nodes", () => {
  it.each([
    ["element", () => document.createElement("div")],
    ["text node", () => document.createTextNode("t")],
    ["comment", () => document.createComment("c")],
    ["fragment", () => document.createDocumentFragment()],
  ] as const)("holds the %s until its handle is collected", (_name, create) => {
    const node = create();

    expect(native.holdNode).toHaveBeenCalledWith(node.__node_id);
    expect(isHeld(node)).toBe(true);
  });

  it.each([
    ["createElement", () => document.createElement(...([] as unknown as [string]))],
    ["createTextNode", () => document.createTextNode(...([] as unknown as [string]))],
    ["createComment", () => document.createComment(...([] as unknown as [string]))],
  ] as const)("%s without an argument throws a TypeError", (_name, call) => {
    expect(call).toThrow(TypeError);
  });

  it.each([
    [
      "createTextNode(undefined) reads the data undefined",
      () => document.createTextNode(undefined as never),
      "undefined",
    ],
    [
      "createComment(null) reads the data null",
      () => document.createComment(null as never),
      "null",
    ],
  ] as const)("%s as text", (_name, create, expected) => {
    expect(create().nodeValue).toBe(expected);
  });

  it("creates a parentless text node", () => {
    expect(document.createTextNode("t")).toMatchObject({ kind: "text", text: "t", parent: null });
  });

  it("creates a parentless comment", () => {
    expect(document.createComment("c")).toMatchObject({ kind: "comment", parent: null });
  });
});

describe("createDocumentFragment", () => {
  it("allocates a native fragment node", () => {
    document.createDocumentFragment();

    expect(native.createNode).toHaveBeenCalledWith("#fragment");
  });

  it("returns an empty, parentless fragment", () => {
    const fragment = document.createDocumentFragment();

    expect(fragment).toMatchObject({ kind: "fragment", parent: null, __children: [] });
  });

  it.each([
    ["nodeType", 11],
    ["nodeName", "#document-fragment"],
    ["nodeValue", null],
    ["isConnected", false],
    ["ownerDocument", document],
  ] as const)("reads %s of a fragment", (member, value) => {
    expect(document.createDocumentFragment()[member]).toBe(value);
  });

  it("ignores an assignment to nodeValue", () => {
    const fragment = document.createDocumentFragment();

    fragment.nodeValue = "x";

    expect(fragment.nodeValue).toBeNull();
    expect(native.setAttribute).not.toHaveBeenCalled();
  });

  it.each([
    "addEventListener",
    "removeEventListener",
    "dispatchEvent",
    "querySelector",
    "querySelectorAll",
    "children",
    "getElementById",
  ])("has no %s", (member) => {
    expect(member in document.createDocumentFragment()).toBe(false);
  });

  it("reads textContent from the host", () => {
    expect(document.createDocumentFragment().textContent).toBe("joined");
  });

  it("reads an empty textContent once the host freed it", () => {
    const fragment = document.createDocumentFragment();
    markDestroyed(fragment);

    expect(fragment.textContent).toBe("");
  });

  it("replaces its children with one text node when textContent is assigned", () => {
    const fragment = document.createDocumentFragment();
    fragment.appendChild(createElementNode("div"));

    fragment.textContent = "x";

    expect(fragment.__children.map((child) => child.kind)).toEqual(["text"]);
  });

  it("ignores a textContent assignment once the host freed it", () => {
    const fragment = document.createDocumentFragment();
    markDestroyed(fragment);

    fragment.textContent = "x";

    expect(native.setAttribute).not.toHaveBeenCalled();
  });
});

describe("a node inside a fragment", () => {
  const inside = () => {
    const fragment = document.createDocumentFragment();
    const child = createElementNode("div");
    fragment.appendChild(child);
    parents.set(child.__node_id, fragment.__node_id);
    return { fragment, child };
  };

  it("reads the fragment as parentNode", () => {
    const { fragment, child } = inside();

    expect(child.parentNode).toBe(fragment);
  });

  it("has no offsetParent", () => {
    expect(inside().child.offsetParent).toBeNull();
  });

  it("reads null as parentElement", () => {
    expect(inside().child.parentElement).toBeNull();
  });
});

describe("queries", () => {
  it("finds the element the host names with querySelector", () => {
    const el = createElementNode("div");
    found = el.__node_id;

    expect(document.querySelector("div")).toBe(el);
    expect(native.querySelector).toHaveBeenCalledExactlyOnceWith(DOCUMENT, "div");
  });

  it("gives null when the host finds nothing", () => {
    expect(document.querySelector("div")).toBeNull();
  });

  it("lists the elements the host names with querySelectorAll", () => {
    const [first, second] = [createElementNode("div"), createElementNode("div")];
    foundAll = [first.__node_id, second.__node_id];

    expect([...document.querySelectorAll("div")]).toEqual([first, second]);
    expect(native.querySelectorAll).toHaveBeenCalledExactlyOnceWith(DOCUMENT, "div");
  });

  it("finds the mount container when it matches", () => {
    found = 0;

    expect(document.querySelector("div")).toBe(rootElement());
  });

  it.each(["querySelector", "querySelectorAll"] as const)("%s needs an argument", (method) => {
    expect(() => document[method](...([] as unknown as [string]))).toThrow(TypeError);
  });
});
