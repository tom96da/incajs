// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it, vi } from "vitest";

import { installMembersHost, nativeStub } from "../../tests/setup.mts";
import { createIncaElement } from "./handle.mts";
import { markFreed } from "./lifecycle.mts";
import { scrollIntoViewArgs, scrollPosition } from "./scroll.mts";
import type { ScrollOptions } from "./scroll.mts";

function installHost(scroll: readonly [number, number, number, number] | null = null) {
  const host = {
    scrollOf: vi.fn<() => readonly [number, number, number, number] | null>(() => scroll),
    scrollNode: vi.fn<(nodeId: number, left: number | null, top: number | null) => void>(),
    getAttribute: vi.fn<() => null>(() => null),
    layoutOf: vi.fn<() => null>(() => null),
  };
  vi.stubGlobal("__inca_native__", nativeStub(host as never));
  return host;
}

describe("scrollPosition", () => {
  it("sets both axes from the number form", () => {
    const host = installHost();
    scrollPosition(createIncaElement(7, "div"), [30, 40], false);
    expect(host.scrollNode).toHaveBeenCalledWith(7, 30, 40);
  });

  it("sets both axes from the options form and ignores behavior", () => {
    const host = installHost();
    const el = createIncaElement(7, "div");
    scrollPosition(el, [{ left: 5, top: 9, behavior: "smooth" }], false);
    expect(host.scrollNode).toHaveBeenCalledWith(7, 5, 9);
  });

  it("passes null for an axis the call leaves out", () => {
    const host = installHost();
    const el = createIncaElement(7, "div");
    scrollPosition(el, [undefined, 12], false);
    expect(host.scrollNode).toHaveBeenLastCalledWith(7, null, 12);
    scrollPosition(el, [{ top: 3 }], false);
    expect(host.scrollNode).toHaveBeenLastCalledWith(7, null, 3);
  });

  it("adds to the current offset for a relative call", () => {
    const host = installHost([10, 20, 100, 200]);
    const el = createIncaElement(7, "div");
    scrollPosition(el, [5, -4], true);
    expect(host.scrollNode).toHaveBeenLastCalledWith(7, 15, 16);
  });

  it("turns non-finite values into 0", () => {
    const host = installHost();
    const el = createIncaElement(7, "div");
    scrollPosition(el, [NaN, Infinity], false);
    expect(host.scrollNode).toHaveBeenLastCalledWith(7, 0, 0);
  });

  it("starts a relative call from 0 when there is no record", () => {
    const host = installHost(null);
    scrollPosition(createIncaElement(7, "div"), [4], true);
    expect(host.scrollNode).toHaveBeenLastCalledWith(7, 4, null);
  });

  it("ignores a call on a removed element", () => {
    const host = installHost([1, 2, 3, 4]);
    const el = createIncaElement(7, "div");
    markFreed([el.__node_id]);
    scrollPosition(el, [1, 1], true);
    expect(host.scrollNode).not.toHaveBeenCalled();
  });
});

describe("scrollIntoViewArgs", () => {
  it("aligns to the start when no argument or true is given", () => {
    expect(scrollIntoViewArgs()).toEqual({ block: "start", inline: "nearest" });
    expect(scrollIntoViewArgs(true)).toEqual({ block: "start", inline: "nearest" });
  });

  it("aligns to the end when false is given", () => {
    expect(scrollIntoViewArgs(false)).toEqual({ block: "end", inline: "nearest" });
  });

  it("takes each axis from the options object", () => {
    expect(scrollIntoViewArgs({ block: "center", inline: "end" })).toEqual({
      block: "center",
      inline: "end",
    });
  });

  it("defaults a missing axis to its own default", () => {
    expect(scrollIntoViewArgs({ inline: "center" })).toEqual({ block: "start", inline: "center" });
    expect(scrollIntoViewArgs({ block: "end" })).toEqual({ block: "end", inline: "nearest" });
    expect(scrollIntoViewArgs({})).toEqual({ block: "start", inline: "nearest" });
  });

  it("falls back to the axis default for an unknown string", () => {
    const unknown = { block: "bottom", inline: "left" } as unknown as Parameters<
      typeof scrollIntoViewArgs
    >[0];
    expect(scrollIntoViewArgs(unknown)).toEqual({ block: "start", inline: "nearest" });
  });

  it("ignores behavior", () => {
    expect(scrollIntoViewArgs({ block: "end", behavior: "smooth" })).toEqual({
      block: "end",
      inline: "nearest",
    });
  });
});

describe("createIncaElement scroll members", () => {
  it("read zeros with no record", () => {
    installMembersHost();
    const el = createIncaElement(7, "div");
    expect([el.scrollLeft, el.scrollTop, el.scrollWidth, el.scrollHeight]).toEqual([0, 0, 0, 0]);
  });

  it("report scrollWidth and scrollHeight as client size plus the maximum offset", () => {
    installMembersHost({ 7: [0, 0, 20, 10, 0, 0, 0, 0, 0] }, [0, 0, 30, 60]);
    const el = createIncaElement(7, "div");
    expect([el.scrollWidth, el.scrollHeight]).toEqual([50, 70]);
  });

  it("read zeros once removed", () => {
    installMembersHost({}, [5, 6, 7, 8]);
    const el = createIncaElement(7, "div");
    markFreed([el.__node_id]);
    expect([el.scrollTop, el.scrollWidth]).toEqual([0, 0]);
  });
});

describe("createIncaElement scroll setters and methods", () => {
  it("writes scrollLeft on the horizontal axis only", () => {
    const host = installMembersHost();
    const el = createIncaElement(7, "div");
    el.scrollLeft = 30;
    expect(host.scrollNode).toHaveBeenLastCalledWith(7, 30, null);
  });

  it("writes scrollTop on the vertical axis only", () => {
    const host = installMembersHost();
    const el = createIncaElement(7, "div");
    el.scrollTop = 40;
    expect(host.scrollNode).toHaveBeenLastCalledWith(7, null, 40);
  });

  it("coerces a numeric string assigned to scrollTop", () => {
    const host = installMembersHost();
    const el = createIncaElement(7, "div");
    el.scrollTop = "40" as unknown as number;
    expect(host.scrollNode).toHaveBeenLastCalledWith(7, null, 40);
  });

  it("reads scrollLeft and scrollTop from the record", () => {
    installMembersHost({}, [10, 20, 100, 200]);
    const el = createIncaElement(7, "div");
    expect([el.scrollLeft, el.scrollTop]).toEqual([10, 20]);
  });

  it("wires scrollTo and scrollBy to the number form", () => {
    const host = installMembersHost({}, [5, 6, 100, 200]);
    const el = createIncaElement(7, "div");
    el.scrollTo(10, 20);
    expect(host.scrollNode).toHaveBeenLastCalledWith(7, 10, 20);
    el.scrollBy(1, 2);
    expect(host.scrollNode).toHaveBeenLastCalledWith(7, 6, 8);
  });

  it("passes null for both axes on scrollTo(null)", () => {
    const host = installMembersHost();
    const el = createIncaElement(7, "div");
    el.scrollTo(null as unknown as ScrollOptions);
    expect(host.scrollNode).toHaveBeenLastCalledWith(7, null, null);
  });
});
