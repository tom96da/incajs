// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import { document } from "../document.mts";
import { HTML_NAMESPACE } from "../ffi/index.mts";
import {
  createCommentNode,
  createElementNode,
  createIncaElement,
  handleOf,
  rootElement,
} from "./handle.mts";
import { elementOf, markDestroyed, markRemoved, nodeOf } from "./lifecycle.mts";
import type { ElementName } from "../ffi/index.mts";
import type { IncaElement, IncaInputElement } from "./types.mts";

/** Installs a fake host whose reads give `stored`, `boxes` and `scroll`, and returns it. */
function installHost(
  stored: string | number | boolean | null = null,
  boxes: Record<number, readonly [number, number, number, number, number, number, number]> = {},
  scroll: readonly [number, number, number, number] | null = null,
) {
  const host = {
    getAttribute: vi.fn<(nodeId: number, key: string) => string | number | boolean | null>(
      () => stored,
    ),
    layoutOf: vi.fn<
      (nodeId: number) => readonly [number, number, number, number, number, number, number] | null
    >((nodeId) => boxes[nodeId] ?? null),
    scrollOf: vi.fn<(nodeId: number) => readonly [number, number, number, number] | null>(
      () => scroll,
    ),
    scrollNode: vi.fn<(nodeId: number, left: number | null, top: number | null) => void>(),
    setAttribute: vi.fn<(nodeId: number, key: string, value: unknown) => void>(),
    focusNode: vi.fn<() => void>(),
    blurNode: vi.fn<() => void>(),
    click: vi.fn<() => void>(),
    setPointerCapture: vi.fn<(nodeId: number, pointerId: number) => void>(),
    releasePointerCapture: vi.fn<(nodeId: number, pointerId: number) => void>(),
    hasPointerCapture: vi.fn<(nodeId: number, pointerId: number) => boolean>(() => true),
    scrollIntoViewNode: vi.fn<(nodeId: number, block: string, inline: string) => void>(),
    createNode: vi.fn<() => number>(() => 60),
    removeChild: vi.fn<(parentId: number, childId: number) => void>(),
    releaseListeners: vi.fn<() => number[]>(() => []),
    holdNode: vi.fn<(nodeId: number) => void>(),
    releaseNode: vi.fn<(nodeId: number) => void>(),
    addEventListener: vi.fn<() => number>(() => 0),
    removeEventListener: vi.fn<() => boolean>(() => true),
    dispatchEvent: vi.fn<(nodeId: number, event: unknown) => boolean>(() => true),
  };
  globalThis.__inca_native__ = nativeStub(host as never);
  return host;
}

/** Installs a fake host that answers every read the members of a new handle make. */
function installCreationHost() {
  const host = {
    createNode: vi.fn<(tag: string) => number>(() => 1),
    setStyle: vi.fn<() => void>(),
    setAttribute: vi.fn<() => void>(),
    getAttribute: vi.fn<() => null>(() => null),
    classList: vi.fn<() => object>(() => ({})),
    styleOf: vi.fn<() => object>(() => ({})),
    attributeNames: vi.fn<() => string[]>(() => []),
    attributeKey: vi.fn<(nodeId: number, name: string) => string>((_id, name) => name),
    datasetTarget: vi.fn<() => object>(() => ({})),
    datasetKeys: vi.fn<() => string[]>(() => []),
    isConnected: vi.fn<() => boolean>(() => false),
    linkedNode: vi.fn<() => null>(() => null),
    childNodeIds: vi.fn<() => number[]>(() => []),
    textContent: vi.fn<() => string>(() => ""),
    innerHtml: vi.fn<() => string>(() => ""),
    outerHtml: vi.fn<() => string>(() => ""),
    layoutOf: vi.fn<() => null>(() => null),
    scrollOf: vi.fn<() => null>(() => null),
    addEventListener: vi.fn<(nodeId: number) => number>(() => 0),
    removeEventListener: vi.fn<(nodeId: number) => boolean>(() => true),
    dispatchEvent: vi.fn<(nodeId: number, event: unknown) => boolean>(() => true),
  };
  globalThis.__inca_native__ = nativeStub(host as never);
  return host;
}

describe("createElement", () => {
  it("allocates a native node for the tag and returns a parentless, childless element", () => {
    const host = installCreationHost();
    const el = createElementNode("div");
    expect(host.createNode).toHaveBeenCalledWith("div");
    expect(el).toEqual({
      id: "",
      className: "",
      classList: {},
      kind: "element",
      tagName: "DIV",
      localName: "div",
      namespaceURI: "http://www.w3.org/1999/xhtml",
      prefix: null,
      parentElement: null,
      firstElementChild: null,
      lastElementChild: null,
      childElementCount: 0,
      previousElementSibling: null,
      nextElementSibling: null,
      parent: null,
      __children: [],
      children: expect.any(Object),
      nodeType: 1,
      nodeName: "DIV",
      nodeValue: null,
      textContent: "",
      innerHTML: "",
      outerHTML: "",
      insertAdjacentHTML: expect.any(Function),
      isConnected: false,
      ownerDocument: document,
      parentNode: null,
      firstChild: null,
      lastChild: null,
      previousSibling: null,
      nextSibling: null,
      childNodes: expect.any(Object),
      hasChildNodes: expect.any(Function),
      appendChild: expect.any(Function),
      insertBefore: expect.any(Function),
      removeChild: expect.any(Function),
      replaceChild: expect.any(Function),
      cloneNode: expect.any(Function),
      compareDocumentPosition: expect.any(Function),
      isSameNode: expect.any(Function),
      isEqualNode: expect.any(Function),
      focus: expect.any(Function),
      blur: expect.any(Function),
      click: expect.any(Function),
      setPointerCapture: expect.any(Function),
      releasePointerCapture: expect.any(Function),
      hasPointerCapture: expect.any(Function),
      contains: expect.any(Function),
      addEventListener: expect.any(Function),
      removeEventListener: expect.any(Function),
      dispatchEvent: expect.any(Function),
      tabIndex: -1,
      querySelector: expect.any(Function),
      querySelectorAll: expect.any(Function),
      matches: expect.any(Function),
      closest: expect.any(Function),
      getAttribute: expect.any(Function),
      setAttribute: expect.any(Function),
      hasAttribute: expect.any(Function),
      removeAttribute: expect.any(Function),
      getAttributeNames: expect.any(Function),
      attributes: expect.anything(),
      style: expect.anything(),
      dataset: expect.anything(),
      getBoundingClientRect: expect.any(Function),
      offsetWidth: 0,
      offsetHeight: 0,
      offsetLeft: 0,
      offsetTop: 0,
      offsetParent: null,
      clientWidth: 0,
      clientHeight: 0,
      clientLeft: 0,
      clientTop: 0,
      scrollLeft: 0,
      scrollTop: 0,
      scrollWidth: 0,
      scrollHeight: 0,
      scrollTo: expect.any(Function),
      scrollBy: expect.any(Function),
      scrollIntoView: expect.any(Function),
      getRootNode: expect.any(Function),
    });
    expect(el.__node_id).toBe(1);
  });
});

describe("createComment", () => {
  it("creates a hidden container in place of a comment", () => {
    const host = installCreationHost();
    const node = createCommentNode();
    expect(host.createNode).toHaveBeenCalledWith("#comment");
    expect(host.setStyle).toHaveBeenCalledWith(1, "display", "none");
    expect(node.kind).toBe("comment");
    expect(node.tagName).toBe("DIV");
  });
});

describe("event target methods", () => {
  it("add a listener under the element's own node id", () => {
    const host = installCreationHost();
    const el = createElementNode("div");

    el.addEventListener("click", vi.fn<() => void>());

    expect(host.addEventListener.mock.calls[0]![0]).toBe(el.__node_id);
  });

  it("remove a listener under the element's own node id", () => {
    const host = installCreationHost();
    const el = createElementNode("div");
    const listener = vi.fn<() => void>();
    el.addEventListener("click", listener);

    el.removeEventListener("click", listener);

    expect(host.removeEventListener.mock.calls[0]![0]).toBe(el.__node_id);
  });

  it("dispatch an event at the element's own node id", () => {
    const host = installCreationHost();
    const el = createElementNode("div");

    el.dispatchEvent({ type: "ping" });

    expect(host.dispatchEvent.mock.calls[0]![0]).toBe(el.__node_id);
  });

  it("register listeners under the comment stand-in's node id", () => {
    const host = installCreationHost();
    host.createNode.mockReturnValueOnce(5);
    const comment = createCommentNode();

    comment.addEventListener("click", vi.fn<() => void>());

    expect(host.addEventListener.mock.calls[0]![0]).toBe(5);
  });
});

describe("createIncaElement tagName", () => {
  it.each([
    ["button", "BUTTON"],
    ["straße", "STRAßE"],
  ])("reads the tag %s in ASCII upper case", (tag, expected) => {
    installHost();

    expect(createIncaElement(1, "element", tag).tagName).toBe(expected);
  });

  it("reads DIV for a comment stand-in", () => {
    installHost();

    expect(createIncaElement(3, "comment").tagName).toBe("DIV");
  });

  it("reads DIV for the mount container", () => {
    globalThis.__inca_native__ = nativeStub({ rootNodeId: () => 90 } as never);

    expect(rootElement().tagName).toBe("DIV");
  });

  it("finds the mount container by its id before any handle of it exists", () => {
    globalThis.__inca_native__ = nativeStub({ rootNodeId: () => 91 } as never);

    expect(handleOf(91)).toBe(rootElement());
  });

  it("reads the tag of a removed element", () => {
    installHost();
    const el = createIncaElement(1, "element", "input");
    markRemoved(el);

    expect(el.tagName).toBe("INPUT");
  });

  it("reads the tag of a freed element", () => {
    installHost();
    const el = createIncaElement(1, "element", "input");
    markDestroyed(el);

    expect(el.tagName).toBe("INPUT");
  });
});

describe("createIncaElement name", () => {
  const SVG_NAMESPACE = "http://www.w3.org/2000/svg";
  const svg = (localName: string): ElementName => [localName, SVG_NAMESPACE, null, localName];
  const membersOf = (el: IncaElement) => [el.tagName, el.localName, el.namespaceURI, el.prefix];

  it("reads the members of a name tuple", () => {
    installHost();

    const el = createIncaElement(1, "element", ["Y", "urn:x", "x", "X:Y"]);

    expect(membersOf(el)).toEqual(["X:Y", "Y", "urn:x", "x"]);
  });

  it("reads a tag string as an HTML name", () => {
    installHost();

    const el = createIncaElement(1, "element", "button");

    expect(membersOf(el)).toEqual(["BUTTON", "button", HTML_NAMESPACE, null]);
  });

  it("gives a tag string and the equal tuple the same members", () => {
    installHost();

    const tuple = createIncaElement(1, "element", ["svg", HTML_NAMESPACE, null, "SVG"]);

    expect(membersOf(tuple)).toEqual(membersOf(createIncaElement(2, "element", "svg")));
  });

  it("makes an HTML div without a name", () => {
    installHost();

    expect(membersOf(createIncaElement(1, "element"))).toEqual([
      "DIV",
      "div",
      HTML_NAMESPACE,
      null,
    ]);
  });

  it.each(["tagName", "localName", "namespaceURI", "prefix"] as const)(
    "throws in strict code on an assignment to %s",
    (member) => {
      installHost();
      const el = createIncaElement(1, "element", "div");

      expect(() => ((el as unknown as Record<string, unknown>)[member] = "x")).toThrow(TypeError);
    },
  );

  it("gives an SVG input no value", () => {
    installHost();

    expect("value" in createIncaElement(1, "element", svg("input"))).toBe(false);
  });

  it("gives an SVG template no content", () => {
    installHost();

    expect("content" in createIncaElement(1, "element", svg("template"))).toBe(false);
  });

  it("reads -1 as the default tab index of an SVG button", () => {
    installHost();

    expect(createIncaElement(1, "element", svg("button")).tabIndex).toBe(-1);
  });
});

describe("createIncaElement tabIndex", () => {
  it("reads -1 for a comment stand-in", () => {
    installHost();
    expect(createIncaElement(3, "comment").tabIndex).toBe(-1);
  });

  it("reads the stored tabindex attribute", () => {
    const host = installHost("4");
    expect(createIncaElement(1, "element", "div").tabIndex).toBe(4);
    expect(host.getAttribute).toHaveBeenCalledWith(1, "tabindex");
  });

  it("writes the value as a 32-bit integer", () => {
    const host = installHost();
    const el = createIncaElement(1, "element", "div");

    el.tabIndex = -1.5;
    expect(host.setAttribute).toHaveBeenLastCalledWith(1, "tabindex", -1);
  });
});

describe("createIncaElement value", () => {
  /** Installs a fake host whose `inputValue` gives `text`, and returns it. */
  function installInput(text = "") {
    const host = {
      inputValue: vi.fn<(nodeId: number) => string>(() => text),
      setInputValue: vi.fn<(nodeId: number, value: string) => void>(),
    };
    globalThis.__inca_native__ = nativeStub(host as never);
    return host;
  }

  it("reads the text of an input", () => {
    const host = installInput("abc");
    expect((createIncaElement(1, "element", "input") as IncaInputElement).value).toBe("abc");
    expect(host.inputValue).toHaveBeenCalledWith(1);
  });

  it.each<[unknown, string]>([
    [5, "5"],
    [null, ""],
  ])("assigns the value of an input as a string (%o)", (value, expected) => {
    const host = installInput();
    (createIncaElement(1, "element", "input") as IncaInputElement).value = value as string;
    expect(host.setInputValue).toHaveBeenCalledWith(1, expected);
  });

  it("reads an empty text after the node is removed", () => {
    const host = installInput("abc");
    const el = createIncaElement(1, "element", "input");
    markDestroyed(el);
    expect((el as IncaInputElement).value).toBe("");
    expect(host.inputValue).not.toHaveBeenCalled();
  });

  it("ignores an assignment after the node is removed", () => {
    const host = installInput();
    const el = createIncaElement(1, "element", "input");
    markDestroyed(el);
    (el as IncaInputElement).value = "a";
    expect(host.setInputValue).not.toHaveBeenCalled();
  });

  it("gives other elements no value property", () => {
    installInput();
    expect("value" in createIncaElement(1, "element", "div")).toBe(false);
  });
});

describe("createIncaElement after the node is removed", () => {
  /** A handle that left the tree, over a host whose attribute reads give `stored`. */
  function removed(stored: string | number | boolean | null = null, tag: "div" | "input" = "div") {
    const host = installHost(stored);
    const el = createIncaElement(1, "element", tag);
    markRemoved(el);
    return { host, el };
  }

  it("reads its id from the host", () => {
    const { host, el } = removed("a");

    expect(el.id).toBe("a");
    expect(host.getAttribute).toHaveBeenCalledWith(1, "id");
  });

  it("reads its tabindex from the host", () => {
    expect(removed("4").el.tabIndex).toBe(4);
  });

  it("reads the text of an input from the host", () => {
    const host = {
      inputValue: vi.fn<(nodeId: number) => string>(() => "abc"),
      setInputValue: vi.fn<(nodeId: number, value: string) => void>(),
    };
    globalThis.__inca_native__ = nativeStub(host as never);
    const el = createIncaElement(1, "element", "input");
    markRemoved(el);

    expect((el as IncaInputElement).value).toBe("abc");
  });

  it.each<["id" | "tabIndex", unknown, [number, string, unknown]]>([
    ["id", "b", [1, "id", "b"]],
    ["tabIndex", 3, [1, "tabindex", 3]],
  ])("writes %s through the host", (member, value, call) => {
    const { host, el } = removed();

    (el as unknown as Record<string, unknown>)[member] = value;

    expect(host.setAttribute).toHaveBeenCalledExactlyOnceWith(...call);
  });

  it("writes the text of an input through the host", () => {
    const host = { setInputValue: vi.fn<(nodeId: number, value: string) => void>() };
    globalThis.__inca_native__ = nativeStub(host as never);
    const el = createIncaElement(1, "element", "input");
    markRemoved(el);

    (el as IncaInputElement).value = "t";

    expect(host.setInputValue).toHaveBeenCalledWith(1, "t");
  });

  it.each(["addEventListener", "removeEventListener"] as const)("ignores %s", (method) => {
    const { host, el } = removed();

    (el[method] as (type: string, listener: () => void) => void)("click", () => {});

    expect(host[method]).not.toHaveBeenCalled();
  });

  it("passes dispatchEvent to the host, which checks the argument", () => {
    const { host, el } = removed();
    host.dispatchEvent.mockImplementation(() => {
      throw new TypeError("not an event");
    });

    expect(() => el.dispatchEvent({} as never)).toThrow(TypeError);
    expect(host.dispatchEvent).toHaveBeenCalledWith(1, {});
  });

  it.each([
    ["focus", "focusNode"],
    ["click", "click"],
    ["scrollIntoView", "scrollIntoViewNode"],
  ] as const)("ignores %s", (method, call) => {
    const { host, el } = removed();

    el[method]();

    expect(host[call]).not.toHaveBeenCalled();
  });

  it("reads zero box members without asking the host", () => {
    const host = installHost(null, { 1: [10, 20, 30, 40, 2, 0, 0] });
    const el = createIncaElement(1, "element", "div");
    markRemoved(el);

    expect([el.offsetWidth, el.clientHeight, el.offsetLeft]).toEqual([0, 0, 0]);
    expect(host.layoutOf).not.toHaveBeenCalled();
  });

  it("reads zero scroll members without asking the host", () => {
    const host = installHost(null, {}, [5, 6, 7, 8]);
    const el = createIncaElement(1, "element", "div");
    markRemoved(el);

    expect([el.scrollTop, el.scrollWidth]).toEqual([0, 0]);
    expect(host.scrollOf).not.toHaveBeenCalled();
  });

  it("ignores scrollTo", () => {
    const host = installHost(null, {}, [5, 6, 7, 8]);
    const el = createIncaElement(1, "element", "div");
    markRemoved(el);

    el.scrollTo(1, 2);

    expect(host.scrollNode).not.toHaveBeenCalled();
  });
});

describe("createIncaElement registration", () => {
  it("registers an element handle", () => {
    installHost();
    const el = createIncaElement(7, "element", "div");

    expect(elementOf(7)).toBe(el);
  });

  it("leaves a comment stand-in out of elementOf", () => {
    installHost();
    createIncaElement(8, "comment");

    expect(elementOf(8)).toBeUndefined();
  });

  it("registers a comment stand-in as a node", () => {
    installHost();
    const comment = createIncaElement(8, "comment");

    expect(nodeOf(8)).toBe(comment);
  });
});

describe("createIncaElement dataset", () => {
  it("gives an element a dataset", () => {
    installHost();

    expect("dataset" in createIncaElement(8, "element", "div")).toBe(true);
  });
});

describe("createIncaElement destroyed attributes", () => {
  it("ignores setAttribute with an invalid name on a destroyed element", () => {
    const host = installHost();
    const el = createIncaElement(8, "element", "div");
    markDestroyed(el);

    expect(() => el.setAttribute("a b", "v")).not.toThrow();
    expect(host.setAttribute).not.toHaveBeenCalled();
  });
});

describe("createIncaElement focus and scrollIntoView", () => {
  it.each([
    ["no options", undefined, false],
    ["a truthy preventScroll", { preventScroll: 1 }, true],
  ])("passes preventScroll as a boolean for %s", (_name, options, expected) => {
    const host = installHost();
    const el = createIncaElement(3, "element", "div");

    el.focus(options as { preventScroll?: boolean } | undefined);
    expect(host.focusNode).toHaveBeenLastCalledWith(3, expected);
  });

  it("forwards the normalised alignments of scrollIntoView to the host", () => {
    const host = installHost();
    const el = createIncaElement(3, "element", "div");

    el.scrollIntoView({ block: "center", inline: "end", behavior: "smooth" });
    expect(host.scrollIntoViewNode).toHaveBeenLastCalledWith(3, "center", "end");
  });

  it("ignores focus and scrollIntoView once freed", () => {
    const host = installHost();
    const el = createIncaElement(3, "element", "div");
    markDestroyed(el);

    el.focus({ preventScroll: true });
    el.scrollIntoView(false);
    expect(host.focusNode).not.toHaveBeenCalled();
    expect(host.scrollIntoViewNode).not.toHaveBeenCalled();
  });
});

describe("createIncaElement blur and click", () => {
  it("forwards blur of a freed element to the host", () => {
    const host = installHost();
    const el = createIncaElement(4, "element", "button");
    markDestroyed(el);

    el.blur();
    expect(host.blurNode).toHaveBeenCalledWith(4);
  });

  it("ignores click once freed", () => {
    const host = installHost();
    const el = createIncaElement(4, "element", "button");
    markDestroyed(el);

    el.click();
    expect(host.click).not.toHaveBeenCalled();
  });
});

describe("createIncaElement pointer capture", () => {
  const methods = ["setPointerCapture", "releasePointerCapture", "hasPointerCapture"] as const;

  it.each(methods)("forwards %s to native with the node id and the pointer id", (method) => {
    const host = installHost();
    const el = createIncaElement(4, "element", "div");

    el[method](1);

    expect(host[method]).toHaveBeenCalledWith(4, 1);
  });

  it("returns what the host answers to hasPointerCapture", () => {
    const host = installHost();
    host.hasPointerCapture.mockReturnValue(true);

    expect(createIncaElement(4, "element", "div").hasPointerCapture(1)).toBe(true);
  });

  describe("after the element is freed", () => {
    function freed() {
      const host = installHost();
      const el = createIncaElement(4, "element", "div");
      markDestroyed(el);
      return { host, el };
    }

    it("still throws the host's error from setPointerCapture", () => {
      const { host, el } = freed();
      host.setPointerCapture.mockImplementation(() => {
        throw new DOMException("gone", "InvalidStateError");
      });

      expect(() => el.setPointerCapture(1)).toThrow(
        expect.objectContaining({ name: "InvalidStateError" }),
      );
      expect(host.setPointerCapture).toHaveBeenCalledWith(4, 1);
    });

    it("answers hasPointerCapture with the host's answer", () => {
      const { host, el } = freed();
      host.hasPointerCapture.mockReturnValue(false);

      expect(el.hasPointerCapture(1)).toBe(false);
      expect(host.hasPointerCapture).toHaveBeenCalledWith(4, 1);
    });

    it("still forwards releasePointerCapture without throwing", () => {
      const { host, el } = freed();

      expect(() => el.releasePointerCapture(1)).not.toThrow();
      expect(host.releasePointerCapture).toHaveBeenCalledWith(4, 1);
    });
  });
});

describe("createIncaElement id", () => {
  it.each([
    [null, ""],
    ["a", "a"],
    [5, "5"],
  ] as const)("reads a stored %j as %j", (stored, expected) => {
    const host = installHost(stored);
    expect(createIncaElement(1, "element", "div").id).toBe(expected);
    expect(host.getAttribute).toHaveBeenCalledWith(1, "id");
  });

  it.each([
    [7, "7"],
    [null, "null"],
  ] as const)("writes %j as the attribute %j", (value, expected) => {
    const host = installHost();
    const el = createIncaElement(1, "element", "div");
    el.id = value as unknown as string;
    expect(host.setAttribute).toHaveBeenLastCalledWith(1, "id", expected);
  });

  it("reads an empty string after the node is freed", () => {
    const host = installHost("a");
    const el = createIncaElement(1, "element", "div");
    markDestroyed(el);
    expect(el.id).toBe("");
    expect(host.getAttribute).not.toHaveBeenCalled();
  });

  it("ignores an id write after the node is freed", () => {
    const host = installHost();
    const el = createIncaElement(1, "element", "div");
    markDestroyed(el);
    el.id = "b";
    expect(host.setAttribute).not.toHaveBeenCalled();
  });

  it("keeps the node id in a read-only, non-enumerable __node_id", () => {
    installHost();
    const el = createIncaElement(9, "element", "div");
    expect(el.__node_id).toBe(9);
    expect(Object.keys(el)).not.toContain("__node_id");
    expect(() => {
      (el as { __node_id: number }).__node_id = 1;
    }).toThrow(TypeError);
  });
});

describe("rootElement wheel listeners", () => {
  /** The `passive` argument that `element` registers a `wheel` listener with. */
  function registeredPassive(element: IncaElement, options?: { passive: boolean }) {
    const addEventListener = vi.fn<(...args: unknown[]) => number>(() => 0);
    globalThis.__inca_native__ = nativeStub({ addEventListener } as never);
    element.addEventListener("wheel", () => {}, options);
    return addEventListener.mock.calls[0]![4];
  }

  function root(id: number): IncaElement {
    globalThis.__inca_native__ = nativeStub({ rootNodeId: () => id } as never);
    return rootElement();
  }

  it("are passive by default", () => {
    expect(registeredPassive(root(70))).toBe(true);
  });

  it("are not passive when the options say so", () => {
    expect(registeredPassive(root(71), { passive: false })).toBe(false);
  });

  it("are not passive on an inner element", () => {
    expect(registeredPassive(createIncaElement(72, "element", "div"))).toBe(false);
  });
});
