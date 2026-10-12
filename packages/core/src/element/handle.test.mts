// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { setFlagsFromString } from "node:v8";
import { runInNewContext } from "node:vm";

import { beforeEach, describe, expect, it, vi } from "vitest";

import { activeElement, createIncaElement } from "./handle.mts";
import { elementOf, markDestroyed, recordedCount, unregisterElement } from "./lifecycle.mts";
import type { IncaCore } from "../rendererCore.mts";
import type { ScrollOptions } from "./scroll.mts";

function makeCore(
  stored: string | number | boolean | null = null,
  boxes: Record<number, readonly [number, number, number, number, number, number, number]> = {},
  scroll: readonly [number, number, number, number] | null = null,
): IncaCore {
  return {
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
    focus: vi.fn<() => void>(),
    blur: vi.fn<() => void>(),
    click: vi.fn<() => void>(),
    scrollIntoViewNode: vi.fn<(nodeId: number, block: string, inline: string) => void>(),
    createEventTarget: () => ({}),
  } as unknown as IncaCore;
}

describe("createIncaElement tabIndex", () => {
  it("reads -1 for a comment stand-in", () => {
    expect(createIncaElement(makeCore(), 3, "comment").tabIndex).toBe(-1);
  });

  it("reads the stored tabindex attribute", () => {
    const core = makeCore("4");
    expect(createIncaElement(core, 1, "element", "div").tabIndex).toBe(4);
    expect(core.getAttribute).toHaveBeenCalledWith(1, "tabindex");
  });

  it("writes the value as a 32-bit integer", () => {
    const core = makeCore();
    const el = createIncaElement(core, 1, "element", "div");

    el.tabIndex = -1.5;
    expect(core.setAttribute).toHaveBeenLastCalledWith(1, "tabindex", -1);
  });
});

describe("createIncaElement layout", () => {
  it("reads the rect from the last drawn box", () => {
    const el = createIncaElement(
      makeCore(null, { 1: [10, 20, 30, 40, 2, 0, 0] }),
      1,
      "element",
      "div",
    );
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
    const el = createIncaElement(makeCore(), 1, "element", "div");
    expect([el.offsetWidth, el.offsetHeight, el.offsetLeft, el.offsetTop]).toEqual([0, 0, 0, 0]);
    expect([el.clientWidth, el.clientHeight, el.clientLeft, el.clientTop]).toEqual([0, 0, 0, 0]);
    expect(el.offsetParent).toBeNull();
  });

  it("rounds the offset and client members", () => {
    const el = createIncaElement(
      makeCore(null, { 1: [10.4, 20.6, 30.5, 40.2, 1.6, 0, 0] }),
      1,
      "element",
      "div",
    );
    expect([el.offsetWidth, el.offsetHeight]).toEqual([31, 40]);
    expect([el.offsetLeft, el.offsetTop]).toEqual([10, 21]);
    expect([el.clientLeft, el.clientTop]).toEqual([2, 2]);
    expect([el.clientWidth, el.clientHeight]).toEqual([27, 37]);
  });

  it("reads a position slightly above the parent as 0", () => {
    const core = makeCore(null, {
      1: [100, 50, 200, 100, 0, 0, 0],
      2: [99.7, 49.7, 20, 20, 0, 0, 0],
    });
    const parent = createIncaElement(core, 1, "element", "div");
    const child = createIncaElement(core, 2, "element", "div");
    child.parent = parent;

    expect([child.offsetLeft, child.offsetTop]).toEqual([0, 0]);
  });

  it("clamps the client size at zero", () => {
    const el = createIncaElement(makeCore(null, { 1: [0, 0, 2, 2, 5, 0, 0] }), 1, "element", "div");
    expect([el.clientWidth, el.clientHeight]).toEqual([0, 0]);
  });

  it("measures offsets from the offsetParent", () => {
    const core = makeCore(null, {
      1: [100, 50, 200, 100, 0, 0, 0],
      2: [110.4, 60.6, 20, 20, 0, 0, 0],
    });
    const parent = createIncaElement(core, 1, "element", "div");
    const child = createIncaElement(core, 2, "element", "div");
    child.parent = parent;

    expect(child.offsetParent).toBe(parent);
    expect([child.offsetLeft, child.offsetTop]).toEqual([10, 11]);
  });

  it("gives a null offsetParent when the parent has no record", () => {
    const core = makeCore(null, { 2: [110, 60, 20, 20, 0, 0, 0] });
    const child = createIncaElement(core, 2, "element", "div");
    child.parent = createIncaElement(core, 1, "element", "div");

    expect(child.offsetParent).toBeNull();
    expect([child.offsetLeft, child.offsetTop]).toEqual([110, 60]);
  });

  it("gives a null offsetParent to the root wrapper", () => {
    const el = createIncaElement(
      makeCore(null, { 1: [5.4, 6, 10, 10, 0, 0, 0] }),
      1,
      "element",
      "div",
    );
    expect(el.offsetParent).toBeNull();
    expect([el.offsetLeft, el.offsetTop]).toEqual([5, 6]);
  });

  it("reads zeros for a destroyed element", () => {
    const el = createIncaElement(
      makeCore(null, { 1: [10, 20, 30, 40, 2, 0, 0] }),
      1,
      "element",
      "div",
    );
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
  it("registers an element and leaves a comment stand-in out", () => {
    const el = createIncaElement(makeCore(), 7, "element", "div");
    createIncaElement(makeCore(), 8, "comment");

    expect([elementOf(7), elementOf(8) !== undefined]).toEqual([el, false]);
  });
});

describe("createIncaElement focus and scrollIntoView", () => {
  it("forwards preventScroll only when the option holds it", () => {
    const core = makeCore();
    const el = createIncaElement(core, 3, "element", "div");

    el.focus();
    expect(core.focus).toHaveBeenLastCalledWith(3);

    el.focus({});
    expect(core.focus).toHaveBeenLastCalledWith(3);

    el.focus({ preventScroll: false });
    expect(core.focus).toHaveBeenLastCalledWith(3, false);

    el.focus({ preventScroll: true });
    expect(core.focus).toHaveBeenLastCalledWith(3, true);
  });

  it("forwards the normalised alignments of scrollIntoView to the host", () => {
    const core = makeCore();
    const el = createIncaElement(core, 3, "element", "div");

    el.scrollIntoView({ block: "center", inline: "end", behavior: "smooth" });
    expect(core.scrollIntoViewNode).toHaveBeenLastCalledWith(3, "center", "end");
  });

  it("ignores focus and scrollIntoView once destroyed", () => {
    const core = makeCore();
    const el = createIncaElement(core, 3, "element", "div");
    markDestroyed(el);

    el.focus({ preventScroll: true });
    el.scrollIntoView(false);
    expect(core.focus).not.toHaveBeenCalled();
    expect(core.scrollIntoViewNode).not.toHaveBeenCalled();
  });
});

describe("createIncaElement scroll members", () => {
  it("read zeros with no record", () => {
    const el = createIncaElement(makeCore(), 7, "element", "div");
    expect([el.scrollLeft, el.scrollTop, el.scrollWidth, el.scrollHeight]).toEqual([0, 0, 0, 0]);
  });

  it("report scrollWidth and scrollHeight as client size plus the maximum offset", () => {
    const el = createIncaElement(
      makeCore(null, { 7: [0, 0, 20, 10, 0, 0, 0] }, [0, 0, 30, 60]),
      7,
      "element",
      "div",
    );
    expect([el.scrollWidth, el.scrollHeight]).toEqual([50, 70]);
  });

  it("read zeros once removed", () => {
    const core = makeCore(null, {}, [5, 6, 7, 8]);
    const el = createIncaElement(core, 7, "element", "div");
    markDestroyed(el);
    expect([el.scrollTop, el.scrollWidth]).toEqual([0, 0]);
  });
});

describe("createIncaElement scroll setters and methods", () => {
  it("writes scrollLeft on the horizontal axis only", () => {
    const core = makeCore();
    const el = createIncaElement(core, 7, "element", "div");
    el.scrollLeft = 30;
    expect(core.scrollNode).toHaveBeenLastCalledWith(7, 30, null);
  });

  it("writes scrollTop on the vertical axis only", () => {
    const core = makeCore();
    const el = createIncaElement(core, 7, "element", "div");
    el.scrollTop = 40;
    expect(core.scrollNode).toHaveBeenLastCalledWith(7, null, 40);
  });

  it("coerces a numeric string assigned to scrollTop", () => {
    const core = makeCore();
    const el = createIncaElement(core, 7, "element", "div");
    el.scrollTop = "40" as unknown as number;
    expect(core.scrollNode).toHaveBeenLastCalledWith(7, null, 40);
  });

  it("reads scrollLeft and scrollTop from the record", () => {
    const el = createIncaElement(makeCore(null, {}, [10, 20, 100, 200]), 7, "element", "div");
    expect([el.scrollLeft, el.scrollTop]).toEqual([10, 20]);
  });

  it("wires scrollTo and scrollBy to the number form", () => {
    const core = makeCore(null, {}, [5, 6, 100, 200]);
    const el = createIncaElement(core, 7, "element", "div");
    el.scrollTo(10, 20);
    expect(core.scrollNode).toHaveBeenLastCalledWith(7, 10, 20);
    el.scrollBy(1, 2);
    expect(core.scrollNode).toHaveBeenLastCalledWith(7, 6, 8);
  });

  it("passes null for both axes on scrollTo(null)", () => {
    const core = makeCore();
    const el = createIncaElement(core, 7, "element", "div");
    el.scrollTo(null as unknown as ScrollOptions);
    expect(core.scrollNode).toHaveBeenLastCalledWith(7, null, null);
  });
});

describe("createIncaElement blur and click", () => {
  it("forwards blur and click of a destroyed element to the host", () => {
    const core = makeCore();
    const el = createIncaElement(core, 4, "element", "button");
    markDestroyed(el);

    el.blur();
    el.click();
    expect(core.blur).toHaveBeenCalledWith(4);
    expect(core.click).toHaveBeenCalledWith(4);
  });
});

describe("createIncaElement registry", () => {
  setFlagsFromString("--expose-gc");
  const gc = runInNewContext("gc") as () => void;

  /** Each round runs in its own timer tick, since a deref keeps its target alive until the job ends. */
  async function collectGarbage(): Promise<void> {
    for (let i = 0; i < 10; i++) {
      gc();
      await new Promise((resolve) => setTimeout(resolve, 5));
    }
  }

  it("releases the entry of an element nothing references", async () => {
    const core = makeCore();
    await collectGarbage();
    const before = recordedCount();
    (() => createIncaElement(core, 31, "element", "div"))();
    expect(recordedCount()).toBe(before + 1);

    await collectGarbage();

    expect(recordedCount()).toBe(before);
  });

  it("keeps the entry of a newer handle when an older one with the same id is collected", async () => {
    const core = makeCore();
    const first = (() => new WeakRef(createIncaElement(core, 33, "element", "div")))();
    const el = createIncaElement(core, 33, "element", "div");
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

const focused = vi.fn<() => number | null>(() => null);

const focusCore = {
  createEventTarget: vi.fn<() => object>(() => ({})),
} as unknown as IncaCore;

describe("activeElement", () => {
  beforeEach(() => {
    for (const id of [21, 22]) unregisterElement(id);
    focused.mockReset();
    focused.mockReturnValue(null);
    globalThis.__inca_native__ = { focusedNode: focused } as never;
  });

  it("returns the registered element of the focused node", () => {
    const el = createIncaElement(focusCore, 21, "element", "div");
    focused.mockReturnValue(21);

    expect(activeElement()).toBe(el);
  });

  it("returns null when no node has focus", () => {
    createIncaElement(focusCore, 22, "element", "div");

    expect(activeElement()).toBeNull();
  });

  it("returns null for a focused id with no registered element", () => {
    focused.mockReturnValue(999);

    expect(activeElement()).toBeNull();
  });
});
