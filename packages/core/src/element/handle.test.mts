// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { installFakeHost } from "../../tests/fakeHost.mts";
import { collectGarbage } from "../../tests/gc.mts";
import { nativeStub } from "../../tests/setup.mts";
import { document } from "../document.mts";
import { HTML_NAMESPACE } from "../ffi/index.mts";
import {
  createCommentNode,
  createElementNode,
  createIncaComment,
  createIncaElement,
  handleOf,
  rootElement,
} from "./handle.mts";
import { elementOf, markFreed, markRemoved, nodeOf } from "./lifecycle.mts";
import type { ElementName } from "../ffi/index.mts";
import type { IncaElement, IncaHTMLElement, IncaInputElement } from "./types.mts";

/** Installs a fake host whose reads give `stored`, `boxes` and `scroll`, and returns it. */
function installHost(
  stored: string | number | boolean | null = null,
  boxes: Record<
    number,
    readonly [number, number, number, number, number, number, number, number, number]
  > = {},
  scroll: readonly [number, number, number, number] | null = null,
) {
  const host = {
    getAttribute: vi.fn<(nodeId: number, key: string) => string | number | boolean | null>(
      () => stored,
    ),
    layoutOf: vi.fn<
      (
        nodeId: number,
      ) => readonly [number, number, number, number, number, number, number, number, number] | null
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
    holdNode: vi.fn<(nodeId: number) => void>(),
    releaseNode: vi.fn<(nodeId: number) => void>(),
    addEventListener: vi.fn<() => number>(() => 0),
    removeEventListener: vi.fn<() => boolean>(() => true),
    dispatchEvent: vi.fn<(nodeId: number, event: unknown) => boolean>(() => true),
  };
  vi.stubGlobal("__inca_native__", nativeStub(host as never));
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
  vi.stubGlobal("__inca_native__", nativeStub(host as never));
  return host;
}

describe("createElement", () => {
  it("allocates a native node for the tag and returns a childless element", () => {
    const host = installCreationHost();
    const el = createElementNode("div");
    expect(host.createNode).toHaveBeenCalledWith("div");
    expect([el.id, el.className, el.kind, el.tagName]).toEqual(["", "", "element", "DIV"]);
    expect(el.ownerDocument).toBe(document);
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
  });

  it.each([
    "tagName",
    "getAttribute",
    "className",
    "classList",
    "style",
    "dataset",
    "innerHTML",
    "tabIndex",
    "getBoundingClientRect",
    "querySelector",
    "children",
  ])("gives a comment no %s", (member) => {
    installCreationHost();

    expect(member in createCommentNode()).toBe(false);
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

    el.dispatchEvent(new Event("x"));

    expect(host.dispatchEvent.mock.calls[0]![0]).toBe(el.__node_id);
  });

  it("register listeners under the comment's node id", () => {
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

    expect(createIncaElement(1, tag).tagName).toBe(expected);
  });

  it("reads DIV for the mount container", () => {
    vi.stubGlobal("__inca_native__", nativeStub({ rootNodeId: () => 90 } as never));

    expect(rootElement().tagName).toBe("DIV");
  });

  it("finds the mount container by its id before any handle of it exists", () => {
    vi.stubGlobal("__inca_native__", nativeStub({ rootNodeId: () => 91 } as never));

    expect(handleOf(91)).toBe(rootElement());
  });

  it("reads the tag of a removed element", () => {
    installHost();
    const el = createIncaElement(1, "input");
    markRemoved([el.__node_id]);

    expect(el.tagName).toBe("INPUT");
  });

  it("reads the tag of a freed element", () => {
    installHost();
    const el = createIncaElement(1, "input");
    markFreed([el.__node_id]);

    expect(el.tagName).toBe("INPUT");
  });
});

describe("createIncaElement name", () => {
  const SVG_NAMESPACE = "http://www.w3.org/2000/svg";
  const svg = (localName: string): ElementName => [localName, SVG_NAMESPACE, null, localName];
  const membersOf = (el: IncaElement) => [el.tagName, el.localName, el.namespaceURI, el.prefix];

  it("reads the members of a name tuple", () => {
    installHost();

    const el = createIncaElement(1, ["Y", "urn:x", "x", "X:Y"]);

    expect(membersOf(el)).toEqual(["X:Y", "Y", "urn:x", "x"]);
  });

  it("reads a tag string as an HTML name", () => {
    installHost();

    const el = createIncaElement(1, "button");

    expect(membersOf(el)).toEqual(["BUTTON", "button", HTML_NAMESPACE, null]);
  });

  it("gives a tag string and the equal tuple the same members", () => {
    installHost();

    const tuple = createIncaElement(1, ["svg", HTML_NAMESPACE, null, "SVG"]);

    expect(membersOf(tuple)).toEqual(membersOf(createIncaElement(2, "svg")));
  });

  it("makes an HTML div without a name", () => {
    installHost();

    expect(membersOf(createIncaElement(1))).toEqual(["DIV", "div", HTML_NAMESPACE, null]);
  });

  it.each(["tagName", "localName", "namespaceURI", "prefix"] as const)(
    "throws in strict code on an assignment to %s",
    (member) => {
      installHost();
      const el = createIncaElement(1, "div");

      expect(() => ((el as unknown as Record<string, unknown>)[member] = "x")).toThrow(TypeError);
    },
  );

  it("gives an SVG input no value", () => {
    installHost();

    expect("value" in createIncaElement(1, svg("input"))).toBe(false);
  });

  it("gives an SVG template no content", () => {
    installHost();

    expect("content" in createIncaElement(1, svg("template"))).toBe(false);
  });

  it("reads -1 as the default tab index of an SVG button", () => {
    installHost();

    expect((createIncaElement(1, svg("button")) as IncaHTMLElement).tabIndex).toBe(-1);
  });
});

describe("createIncaElement tabIndex", () => {
  it("reads the stored tabindex attribute", () => {
    const host = installHost("4");
    expect(createIncaElement(1, "div").tabIndex).toBe(4);
    expect(host.getAttribute).toHaveBeenCalledWith(1, "tabindex");
  });

  it("writes the value as a 32-bit integer", () => {
    const host = installHost();
    const el = createIncaElement(1, "div");

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
    vi.stubGlobal("__inca_native__", nativeStub(host as never));
    return host;
  }

  it("reads the text of an input", () => {
    const host = installInput("abc");
    expect((createIncaElement(1, "input") as IncaInputElement).value).toBe("abc");
    expect(host.inputValue).toHaveBeenCalledWith(1);
  });

  it.each<[unknown, string]>([
    [5, "5"],
    [null, ""],
  ])("assigns the value of an input as a string (%o)", (value, expected) => {
    const host = installInput();
    (createIncaElement(1, "input") as IncaInputElement).value = value as string;
    expect(host.setInputValue).toHaveBeenCalledWith(1, expected);
  });

  it("reads an empty text after the node is removed", () => {
    const host = installInput("abc");
    const el = createIncaElement(1, "input");
    markFreed([el.__node_id]);
    expect((el as IncaInputElement).value).toBe("");
    expect(host.inputValue).not.toHaveBeenCalled();
  });

  it("ignores an assignment after the node is removed", () => {
    const host = installInput();
    const el = createIncaElement(1, "input");
    markFreed([el.__node_id]);
    (el as IncaInputElement).value = "a";
    expect(host.setInputValue).not.toHaveBeenCalled();
  });

  it("gives other elements no value property", () => {
    installInput();
    expect("value" in createIncaElement(1, "div")).toBe(false);
  });
});

describe("createIncaElement after the node is removed", () => {
  /** A handle that left the tree, over a host whose attribute reads give `stored`. */
  function removed(stored: string | number | boolean | null = null, tag: "div" | "input" = "div") {
    const host = installHost(stored);
    const el = createIncaElement(1, tag);
    markRemoved([el.__node_id]);
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
    vi.stubGlobal("__inca_native__", nativeStub(host as never));
    const el = createIncaElement(1, "input");
    markRemoved([el.__node_id]);

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
    vi.stubGlobal("__inca_native__", nativeStub(host as never));
    const el = createIncaElement(1, "input");
    markRemoved([el.__node_id]);

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
    const host = installHost(null, { 1: [10, 20, 30, 40, 2, 0, 0, 0, 0] });
    const el = createIncaElement(1, "div");
    markRemoved([el.__node_id]);

    expect([el.offsetWidth, el.clientHeight, el.offsetLeft]).toEqual([0, 0, 0]);
    expect(host.layoutOf).not.toHaveBeenCalled();
  });

  it("reads zero scroll members without asking the host", () => {
    const host = installHost(null, {}, [5, 6, 7, 8]);
    const el = createIncaElement(1, "div");
    markRemoved([el.__node_id]);

    expect([el.scrollTop, el.scrollWidth]).toEqual([0, 0]);
    expect(host.scrollOf).not.toHaveBeenCalled();
  });

  it("ignores scrollTo", () => {
    const host = installHost(null, {}, [5, 6, 7, 8]);
    const el = createIncaElement(1, "div");
    markRemoved([el.__node_id]);

    el.scrollTo(1, 2);

    expect(host.scrollNode).not.toHaveBeenCalled();
  });
});

describe("createIncaElement registration", () => {
  it("registers an element handle", () => {
    installHost();
    const el = createIncaElement(7, "div");

    expect(elementOf(7)).toBe(el);
  });

  it("leaves a comment out of elementOf", () => {
    installHost();
    createIncaComment(8);

    expect(elementOf(8)).toBeUndefined();
  });

  it("registers a comment as a node", () => {
    installHost();
    const comment = createIncaComment(8);

    expect(nodeOf(8)).toBe(comment);
  });
});

describe("createIncaElement dataset", () => {
  it("gives an element a dataset", () => {
    installHost();

    expect("dataset" in createIncaElement(8, "div")).toBe(true);
  });
});

describe("createIncaElement destroyed attributes", () => {
  it("ignores setAttribute with an invalid name on a destroyed element", () => {
    const host = installHost();
    const el = createIncaElement(8, "div");
    markFreed([el.__node_id]);

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
    const el = createIncaElement(3, "div");

    el.focus(options as { preventScroll?: boolean } | undefined);
    expect(host.focusNode).toHaveBeenLastCalledWith(3, expected);
  });

  it("forwards the normalised alignments of scrollIntoView to the host", () => {
    const host = installHost();
    const el = createIncaElement(3, "div");

    el.scrollIntoView({ block: "center", inline: "end", behavior: "smooth" });
    expect(host.scrollIntoViewNode).toHaveBeenLastCalledWith(3, "center", "end");
  });

  it("ignores focus and scrollIntoView once freed", () => {
    const host = installHost();
    const el = createIncaElement(3, "div");
    markFreed([el.__node_id]);

    el.focus({ preventScroll: true });
    el.scrollIntoView(false);
    expect(host.focusNode).not.toHaveBeenCalled();
    expect(host.scrollIntoViewNode).not.toHaveBeenCalled();
  });
});

describe("createIncaElement blur and click", () => {
  it("forwards blur of a freed element to the host", () => {
    const host = installHost();
    const el = createIncaElement(4, "button");
    markFreed([el.__node_id]);

    el.blur();
    expect(host.blurNode).toHaveBeenCalledWith(4);
  });

  it("ignores click once freed", () => {
    const host = installHost();
    const el = createIncaElement(4, "button");
    markFreed([el.__node_id]);

    el.click();
    expect(host.click).not.toHaveBeenCalled();
  });
});

describe("createIncaElement pointer capture", () => {
  const methods = ["setPointerCapture", "releasePointerCapture", "hasPointerCapture"] as const;

  it.each(methods)("forwards %s to native with the node id and the pointer id", (method) => {
    const host = installHost();
    const el = createIncaElement(4, "div");

    el[method](1);

    expect(host[method]).toHaveBeenCalledWith(4, 1);
  });

  it("returns what the host answers to hasPointerCapture", () => {
    const host = installHost();
    host.hasPointerCapture.mockReturnValue(true);

    expect(createIncaElement(4, "div").hasPointerCapture(1)).toBe(true);
  });

  describe("after the element is freed", () => {
    function freed() {
      const host = installHost();
      const el = createIncaElement(4, "div");
      markFreed([el.__node_id]);
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
    expect(createIncaElement(1, "div").id).toBe(expected);
    expect(host.getAttribute).toHaveBeenCalledWith(1, "id");
  });

  it.each([
    [7, "7"],
    [null, "null"],
  ] as const)("writes %j as the attribute %j", (value, expected) => {
    const host = installHost();
    const el = createIncaElement(1, "div");
    el.id = value as unknown as string;
    expect(host.setAttribute).toHaveBeenLastCalledWith(1, "id", expected);
  });

  it("reads an empty string after the node is freed", () => {
    const host = installHost("a");
    const el = createIncaElement(1, "div");
    markFreed([el.__node_id]);
    expect(el.id).toBe("");
    expect(host.getAttribute).not.toHaveBeenCalled();
  });

  it("ignores an id write after the node is freed", () => {
    const host = installHost();
    const el = createIncaElement(1, "div");
    markFreed([el.__node_id]);
    el.id = "b";
    expect(host.setAttribute).not.toHaveBeenCalled();
  });

  it("keeps the node id in a read-only, non-enumerable __node_id", () => {
    installHost();
    const el = createIncaElement(9, "div");
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
    vi.stubGlobal("__inca_native__", nativeStub({ addEventListener } as never));
    element.addEventListener("wheel", () => {}, options);
    return addEventListener.mock.calls[0]![4];
  }

  function root(id: number): IncaElement {
    vi.stubGlobal("__inca_native__", nativeStub({ rootNodeId: () => id } as never));
    return rootElement();
  }

  it("are passive by default", () => {
    expect(registeredPassive(root(70))).toBe(true);
  });

  it("are not passive when the options say so", () => {
    expect(registeredPassive(root(71), { passive: false })).toBe(false);
  });

  it("are not passive on an inner element", () => {
    expect(registeredPassive(createIncaElement(72, "div"))).toBe(false);
  });
});

describe("the handle of the mount container after a collection", () => {
  it("is read as the parent of a top-level element", async () => {
    const host = installFakeHost();
    const top = createElementNode("div");
    const root = (() => {
      const container = rootElement();
      container.appendChild(top);
      return new WeakRef(container);
    })();

    await collectGarbage(() => root.deref() === undefined);
    expect(root.deref()).toBeUndefined();

    expect(top.parentNode?.__node_id).toBe(host.ROOT);
  });
});
