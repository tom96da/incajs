// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { adoptComment, adoptElement } from "../../tests/adopt.mts";
import { installFakeHost } from "../../tests/fakeHost.mts";
import { collectGarbage } from "../../tests/gc.mts";
import { declareNode, nativeStub } from "../../tests/setup.mts";
import { document } from "../document.mts";
import { createCommentNode, createElementNode, handleOf, rootElement } from "./handle.mts";
import { elementOf, markFreed, markRemoved, nodeOf } from "./lifecycle.mts";
import type { ElementName } from "../ffi/index.mts";
import type { Element, HTMLInputElement } from "./types.mts";

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
    setCharacterData: vi.fn<(nodeId: number, data: string) => void>(),
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
  it("creates a comment without a style", () => {
    const host = installCreationHost();
    const node = createCommentNode();
    expect(host.createNode).toHaveBeenCalledWith("#comment");
    expect(host.setStyle).not.toHaveBeenCalled();
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

describe("handleOf", () => {
  it("finds the mount container by its id before any handle of it exists", () => {
    vi.stubGlobal("__inca_native__", nativeStub({ rootNodeId: () => 91 } as never));

    expect(handleOf(91)).toBe(rootElement());
  });

  it("builds a handle for an id with none, and gives the same object on a second call", () => {
    const first = handleOf(81);

    expect([first?.__node_id, handleOf(81)]).toEqual([81, first]);
    markFreed([81]);
  });

  it("gives undefined for an id the host does not have", () => {
    vi.stubGlobal("__inca_native__", nativeStub({ nodeHandle: () => null } as never));

    expect(handleOf(82)).toBeUndefined();
  });

  it("builds the root container with one call of nodeHandle, as its own handle", () => {
    const real = nativeStub();
    const nodeHandle = vi.fn<(nodeId: number) => ReturnType<typeof real.nodeHandle>>((n) =>
      real.nodeHandle(n),
    );
    vi.stubGlobal("__inca_native__", nativeStub({ rootNodeId: () => 92, nodeHandle } as never));

    expect([handleOf(92), nodeHandle.mock.calls.length]).toEqual([rootElement(), 1]);
    markFreed([92]);
  });

  it.each([
    ["input", "value"],
    ["template", "content"],
  ] as const)("builds an HTML %s with its %s through one call of the host", (tag, member) => {
    const real = nativeStub();
    const nodeHandle = vi.fn<(nodeId: number) => ReturnType<typeof real.nodeHandle>>((id) =>
      real.nodeHandle(id),
    );
    vi.stubGlobal(
      "__inca_native__",
      nativeStub({ nodeHandle, templateContent: () => 84, inputValue: () => "" } as never),
    );
    declareNode(83, { kind: "element", name: tag });
    declareNode(84, { kind: "fragment" });

    const handle = handleOf(83) as Element;

    expect([member in handle, nodeHandle.mock.calls.filter(([id]) => id === 83).length]).toEqual([
      true,
      1,
    ]);
    markFreed([83, 84]);
  });
});

describe("element name", () => {
  const SVG_NAMESPACE = "http://www.w3.org/2000/svg";
  const svg = (localName: string): ElementName => [localName, SVG_NAMESPACE, null, localName];

  it("gives an SVG input no value", () => {
    installHost();

    expect("value" in adoptElement(1, svg("input"))).toBe(false);
  });

  it("gives an SVG template no content", () => {
    installHost();

    expect("content" in adoptElement(1, svg("template"))).toBe(false);
  });
});

describe("element value", () => {
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
    expect((adoptElement(1, "input") as HTMLInputElement).value).toBe("abc");
    expect(host.inputValue).toHaveBeenCalledWith(1);
  });

  it.each<[unknown, string]>([
    [5, "5"],
    [null, ""],
  ])("assigns the value of an input as a string (%o)", (value, expected) => {
    const host = installInput();
    (adoptElement(1, "input") as HTMLInputElement).value = value as string;
    expect(host.setInputValue).toHaveBeenCalledWith(1, expected);
  });

  it("reads an empty text after the node is removed", () => {
    const host = installInput("abc");
    const el = adoptElement(1, "input");
    markFreed([el.__node_id]);
    expect((el as HTMLInputElement).value).toBe("");
    expect(host.inputValue).not.toHaveBeenCalled();
  });

  it("ignores an assignment after the node is removed", () => {
    const host = installInput();
    const el = adoptElement(1, "input");
    markFreed([el.__node_id]);
    (el as HTMLInputElement).value = "a";
    expect(host.setInputValue).not.toHaveBeenCalled();
  });

  it("gives other elements no value property", () => {
    installInput();
    expect("value" in adoptElement(1, "div")).toBe(false);
  });
});

describe("element after the node is removed", () => {
  /** A handle that left the tree, over a host whose attribute reads give `stored`. */
  function removed(stored: string | number | boolean | null = null, tag: "div" | "input" = "div") {
    const host = installHost(stored);
    const el = adoptElement(1, tag);
    markRemoved([el.__node_id]);
    return { host, el };
  }

  it("reads the text of an input from the host", () => {
    const host = {
      inputValue: vi.fn<(nodeId: number) => string>(() => "abc"),
      setInputValue: vi.fn<(nodeId: number, value: string) => void>(),
    };
    vi.stubGlobal("__inca_native__", nativeStub(host as never));
    const el = adoptElement(1, "input");
    markRemoved([el.__node_id]);

    expect((el as HTMLInputElement).value).toBe("abc");
  });

  it("writes the text of an input through the host", () => {
    const host = { setInputValue: vi.fn<(nodeId: number, value: string) => void>() };
    vi.stubGlobal("__inca_native__", nativeStub(host as never));
    const el = adoptElement(1, "input");
    markRemoved([el.__node_id]);

    (el as HTMLInputElement).value = "t";

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
});

describe("element registration", () => {
  it("registers an element handle", () => {
    installHost();
    const el = adoptElement(7, "div");

    expect(elementOf(7)).toBe(el);
  });

  it("leaves a comment out of elementOf", () => {
    installHost();
    adoptComment(8);

    expect(elementOf(8)).toBeUndefined();
  });

  it("registers a comment as a node", () => {
    installHost();
    const comment = adoptComment(8);

    expect(nodeOf(8)).toBe(comment);
  });
});

describe("element dataset", () => {
  it("gives an element a dataset", () => {
    installHost();

    expect("dataset" in adoptElement(8, "div")).toBe(true);
  });
});

describe("element id", () => {
  it("keeps the node id in a read-only, non-enumerable __node_id", () => {
    installHost();
    const el = adoptElement(9, "div");
    expect(el.__node_id).toBe(9);
    expect(Object.keys(el)).not.toContain("__node_id");
    expect(() => {
      (el as { __node_id: number }).__node_id = 1;
    }).toThrow(TypeError);
  });
});

describe("rootElement wheel listeners", () => {
  /** The `passive` argument that `element` registers a `wheel` listener with. */
  function registeredPassive(element: Element, options?: { passive: boolean }) {
    const addEventListener = vi.fn<(...args: unknown[]) => number>(() => 0);
    vi.stubGlobal("__inca_native__", nativeStub({ addEventListener } as never));
    element.addEventListener("wheel", () => {}, options);
    return addEventListener.mock.calls[0]![4];
  }

  function root(id: number): Element {
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
    expect(registeredPassive(adoptElement(72, "div"))).toBe(false);
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
