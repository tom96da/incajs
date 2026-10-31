// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { setFlagsFromString } from "node:v8";
import { runInNewContext } from "node:vm";

import { describe, expect, it, vi } from "vitest";

import { document } from "../document.mts";
import { createIncaElement, rootElement } from "./handle.mts";
import { elementOf, markDestroyed, recordedCount } from "./lifecycle.mts";
import { createTextNode, remove } from "./shadowTree.mts";
import type { ScrollOptions } from "./scroll.mts";
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
    destroyNode: vi.fn<() => number[]>(() => []),
  };
  globalThis.__inca_native__ = host as never;
  return host;
}

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
    globalThis.__inca_native__ = host as never;
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

describe("createIncaElement layout", () => {
  it("reads the rect from the last drawn box", () => {
    installHost(null, { 1: [10, 20, 30, 40, 2, 0, 0] });
    const el = createIncaElement(1, "element", "div");
    const rect = el.getBoundingClientRect();
    expect([
      rect.x,
      rect.y,
      rect.width,
      rect.height,
      rect.left,
      rect.top,
      rect.right,
      rect.bottom,
    ]).toEqual([10, 20, 30, 40, 10, 20, 40, 60]);
  });

  it("gives zero members and a null offsetParent when there is no record", () => {
    installHost();
    const el = createIncaElement(1, "element", "div");
    expect([el.offsetWidth, el.offsetHeight, el.offsetLeft, el.offsetTop]).toEqual([0, 0, 0, 0]);
    expect([el.clientWidth, el.clientHeight, el.clientLeft, el.clientTop]).toEqual([0, 0, 0, 0]);
    expect(el.offsetParent).toBeNull();
  });

  it("rounds the offset and client members", () => {
    installHost(null, { 1: [10.4, 20.6, 30.5, 40.2, 1.6, 0, 0] });
    const el = createIncaElement(1, "element", "div");
    expect([el.offsetWidth, el.offsetHeight]).toEqual([31, 40]);
    expect([el.offsetLeft, el.offsetTop]).toEqual([10, 21]);
    expect([el.clientLeft, el.clientTop]).toEqual([2, 2]);
    expect([el.clientWidth, el.clientHeight]).toEqual([27, 37]);
  });

  it("reads a position slightly above the parent as 0", () => {
    installHost(null, {
      1: [100, 50, 200, 100, 0, 0, 0],
      2: [99.7, 49.7, 20, 20, 0, 0, 0],
    });
    const parent = createIncaElement(1, "element", "div");
    const child = createIncaElement(2, "element", "div");
    child.parent = parent;

    expect([child.offsetLeft, child.offsetTop]).toEqual([0, 0]);
  });

  it("clamps the client size at zero", () => {
    installHost(null, { 1: [0, 0, 2, 2, 5, 0, 0] });
    const el = createIncaElement(1, "element", "div");
    expect([el.clientWidth, el.clientHeight]).toEqual([0, 0]);
  });

  it("measures offsets from the offsetParent", () => {
    installHost(null, {
      1: [100, 50, 200, 100, 0, 0, 0],
      2: [110.4, 60.6, 20, 20, 0, 0, 0],
    });
    const parent = createIncaElement(1, "element", "div");
    const child = createIncaElement(2, "element", "div");
    child.parent = parent;

    expect(child.offsetParent).toBe(parent);
    expect([child.offsetLeft, child.offsetTop]).toEqual([10, 11]);
  });

  it("gives a null offsetParent when the parent has no record", () => {
    installHost(null, { 2: [110, 60, 20, 20, 0, 0, 0] });
    const child = createIncaElement(2, "element", "div");
    child.parent = createIncaElement(1, "element", "div");

    expect(child.offsetParent).toBeNull();
    expect([child.offsetLeft, child.offsetTop]).toEqual([110, 60]);
  });

  it("gives a null offsetParent to the root wrapper", () => {
    installHost(null, { 1: [5.4, 6, 10, 10, 0, 0, 0] });
    const el = createIncaElement(1, "element", "div");
    expect(el.offsetParent).toBeNull();
    expect([el.offsetLeft, el.offsetTop]).toEqual([5, 6]);
  });

  it("reads zeros for a destroyed element", () => {
    installHost(null, { 1: [10, 20, 30, 40, 2, 0, 0] });
    const el = createIncaElement(1, "element", "div");
    markDestroyed(el);
    expect([
      el.getBoundingClientRect().width,
      el.offsetWidth,
      el.clientWidth,
      el.offsetLeft,
    ]).toEqual([0, 0, 0, 0]);
    expect(el.offsetParent).toBeNull();
  });
});

describe("createIncaElement registration", () => {
  it("registers an element handle", () => {
    installHost();
    const el = createIncaElement(7, "element", "div");

    expect(elementOf(7)).toBe(el);
  });

  it("leaves a comment stand-in unregistered", () => {
    installHost();
    createIncaElement(8, "comment");

    expect(elementOf(8)).toBeUndefined();
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

  it("ignores focus and scrollIntoView once destroyed", () => {
    const host = installHost();
    const el = createIncaElement(3, "element", "div");
    markDestroyed(el);

    el.focus({ preventScroll: true });
    el.scrollIntoView(false);
    expect(host.focusNode).not.toHaveBeenCalled();
    expect(host.scrollIntoViewNode).not.toHaveBeenCalled();
  });
});

describe("createIncaElement scroll members", () => {
  it("read zeros with no record", () => {
    installHost();
    const el = createIncaElement(7, "element", "div");
    expect([el.scrollLeft, el.scrollTop, el.scrollWidth, el.scrollHeight]).toEqual([0, 0, 0, 0]);
  });

  it("report scrollWidth and scrollHeight as client size plus the maximum offset", () => {
    installHost(null, { 7: [0, 0, 20, 10, 0, 0, 0] }, [0, 0, 30, 60]);
    const el = createIncaElement(7, "element", "div");
    expect([el.scrollWidth, el.scrollHeight]).toEqual([50, 70]);
  });

  it("read zeros once removed", () => {
    installHost(null, {}, [5, 6, 7, 8]);
    const el = createIncaElement(7, "element", "div");
    markDestroyed(el);
    expect([el.scrollTop, el.scrollWidth]).toEqual([0, 0]);
  });
});

describe("createIncaElement scroll setters and methods", () => {
  it("writes scrollLeft on the horizontal axis only", () => {
    const host = installHost();
    const el = createIncaElement(7, "element", "div");
    el.scrollLeft = 30;
    expect(host.scrollNode).toHaveBeenLastCalledWith(7, 30, null);
  });

  it("writes scrollTop on the vertical axis only", () => {
    const host = installHost();
    const el = createIncaElement(7, "element", "div");
    el.scrollTop = 40;
    expect(host.scrollNode).toHaveBeenLastCalledWith(7, null, 40);
  });

  it("coerces a numeric string assigned to scrollTop", () => {
    const host = installHost();
    const el = createIncaElement(7, "element", "div");
    el.scrollTop = "40" as unknown as number;
    expect(host.scrollNode).toHaveBeenLastCalledWith(7, null, 40);
  });

  it("reads scrollLeft and scrollTop from the record", () => {
    installHost(null, {}, [10, 20, 100, 200]);
    const el = createIncaElement(7, "element", "div");
    expect([el.scrollLeft, el.scrollTop]).toEqual([10, 20]);
  });

  it("wires scrollTo and scrollBy to the number form", () => {
    const host = installHost(null, {}, [5, 6, 100, 200]);
    const el = createIncaElement(7, "element", "div");
    el.scrollTo(10, 20);
    expect(host.scrollNode).toHaveBeenLastCalledWith(7, 10, 20);
    el.scrollBy(1, 2);
    expect(host.scrollNode).toHaveBeenLastCalledWith(7, 6, 8);
  });

  it("passes null for both axes on scrollTo(null)", () => {
    const host = installHost();
    const el = createIncaElement(7, "element", "div");
    el.scrollTo(null as unknown as ScrollOptions);
    expect(host.scrollNode).toHaveBeenLastCalledWith(7, null, null);
  });
});

describe("createIncaElement blur and click", () => {
  it("forwards blur of a destroyed element to the host", () => {
    const host = installHost();
    const el = createIncaElement(4, "element", "button");
    markDestroyed(el);

    el.blur();
    expect(host.blurNode).toHaveBeenCalledWith(4);
  });

  it("ignores click once destroyed", () => {
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

  describe("after the element is destroyed", () => {
    function destroyed() {
      const host = installHost();
      const el = createIncaElement(4, "element", "div");
      markDestroyed(el);
      return { host, el };
    }

    it("still throws the host's error from setPointerCapture", () => {
      const { host, el } = destroyed();
      host.setPointerCapture.mockImplementation(() => {
        throw new DOMException("gone", "InvalidStateError");
      });

      expect(() => el.setPointerCapture(1)).toThrow(
        expect.objectContaining({ name: "InvalidStateError" }),
      );
      expect(host.setPointerCapture).toHaveBeenCalledWith(4, 1);
    });

    it("answers hasPointerCapture with the host's answer", () => {
      const { host, el } = destroyed();
      host.hasPointerCapture.mockReturnValue(false);

      expect(el.hasPointerCapture(1)).toBe(false);
      expect(host.hasPointerCapture).toHaveBeenCalledWith(4, 1);
    });

    it("still forwards releasePointerCapture without throwing", () => {
      const { host, el } = destroyed();

      expect(() => el.releasePointerCapture(1)).not.toThrow();
      expect(host.releasePointerCapture).toHaveBeenCalledWith(4, 1);
    });
  });
});

describe("createIncaElement registry", () => {
  const bun = (globalThis as { Bun?: { gc(sync: boolean): void } }).Bun;
  if (!bun) setFlagsFromString("--expose-gc");
  const gc = bun ? () => bun.gc(true) : (runInNewContext("gc") as () => void);

  /** Each round runs in its own timer tick, since a deref keeps its target alive until the job ends. */
  async function collectGarbage(): Promise<void> {
    for (let i = 0; i < (bun ? 50 : 10); i++) {
      gc();
      await new Promise((resolve) => setTimeout(resolve, 5));
    }
  }

  it("releases the entry of an element nothing references", async () => {
    installHost();
    await collectGarbage();
    const before = recordedCount();
    (() => createIncaElement(31, "element", "div"))();
    expect(recordedCount()).toBe(before + 1);

    await collectGarbage();

    expect(recordedCount()).toBe(before);
  });

  it("keeps a newer handle's entry when an older one with its id is collected", async () => {
    installHost();
    const first = (() => new WeakRef(createIncaElement(33, "element", "div")))();
    const el = createIncaElement(33, "element", "div");
    const tick = (): Promise<unknown> => new Promise((resolve) => setTimeout(resolve, 5));

    // deref() keeps its target alive until the job ends, so each check gets its own job.
    for (let i = 0; i < 40; i++) {
      await tick();
      if (!first.deref()) break;
      await tick();
      gc();
    }
    await tick();
    await tick();

    expect(first.deref()).toBeUndefined();
    expect(elementOf(33)).toBe(el);
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

  it("reads an empty string after the node is destroyed", () => {
    const host = installHost("a");
    const el = createIncaElement(1, "element", "div");
    markDestroyed(el);
    expect(el.id).toBe("");
    expect(host.getAttribute).not.toHaveBeenCalled();
  });

  it("ignores an id write after the node is destroyed", () => {
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

describe("createIncaElement contains", () => {
  const tree = (): IncaElement[] => {
    installHost();
    const [root, child, grandchild] = [41, 42, 43].map((id) =>
      createIncaElement(id, "element", "div"),
    );
    child!.parent = root!;
    root!.children.push(child!);
    grandchild!.parent = child!;
    child!.children.push(grandchild!);
    return [root!, child!, grandchild!];
  };

  it("is true for the element itself", () => {
    const [root] = tree();
    expect(root!.contains(root!)).toBe(true);
  });

  it("is true for a descendant element and a descendant text node", () => {
    const [root, child, grandchild] = tree();
    const text = createTextNode("x");
    text.parent = child!;
    expect([root!.contains(grandchild!), root!.contains(text)]).toEqual([true, true]);
  });

  it("is false for an ancestor and an unrelated element", () => {
    const [root, child] = tree();
    const [other] = tree();
    expect([child!.contains(root!), root!.contains(other!)]).toEqual([false, false]);
  });

  it("follows a removal: the old parent loses the subtree, which keeps its own", () => {
    const [root, child, grandchild] = tree();
    remove(child!);
    expect([root!.contains(child!), child!.contains(grandchild!)]).toEqual([false, true]);
  });

  it("is false for null and undefined", () => {
    const [root] = tree();
    expect([root!.contains(null), root!.contains(undefined as unknown as null)]).toEqual([
      false,
      false,
    ]);
  });

  it.each([
    ["a number", 5],
    ["a plain object with a parent", { parent: null }],
  ])("throws a TypeError for %s", (_name, value) => {
    const [root] = tree();
    expect(() => root!.contains(value as unknown as null)).toThrow(TypeError);
  });
});

describe("rootElement wheel listeners", () => {
  /** The `passive` argument that `element` registers a `wheel` listener with. */
  function registeredPassive(element: IncaElement, options?: { passive: boolean }) {
    const addEventListener = vi.fn<(...args: unknown[]) => number>(() => 0);
    globalThis.__inca_native__ = { addEventListener } as never;
    element.addEventListener("wheel", () => {}, options);
    return addEventListener.mock.calls[0]![3];
  }

  function root(id: number): IncaElement {
    globalThis.__inca_native__ = { rootNodeId: () => id } as never;
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

describe("createIncaElement getRootNode", () => {
  function nested(topId: number) {
    globalThis.__inca_native__ = { rootNodeId: () => 80 } as never;
    const top = createIncaElement(topId, "element", "div");
    const element = createIncaElement(83, "element", "div");
    element.parent = top;
    return { top, element };
  }

  it("gives document under the mount container", () => {
    expect(nested(80).element.getRootNode()).toBe(document);
  });

  it("gives the top element of a detached subtree", () => {
    const { top, element } = nested(81);
    expect(element.getRootNode()).toBe(top);
  });

  it("gives the node itself when alone", () => {
    globalThis.__inca_native__ = { rootNodeId: () => 80 } as never;
    const element = createIncaElement(83, "element", "div");
    expect(element.getRootNode()).toBe(element);
  });
});
