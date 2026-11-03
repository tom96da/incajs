// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import { describe, expect, it } from "vitest";
import type { Plugin } from "vite";

import * as core from "../adapterCore.mts";
import { rejectUnsupported } from "./unsupported.mts";
import type { UnsupportedFeature } from "./unsupported.mts";

const ALPHA: UnsupportedFeature = {
  code: "ERR_INCA_TEST_ALPHA",
  name: "an alpha block",
  matches: (id) => id.includes("type=style"),
  hint: "use a beta block",
};

/** Calls the `transform` hook of the plugin on `id`. */
function transformOf(extra: UnsupportedFeature[], id: string): unknown {
  const plugin = rejectUnsupported(core, extra) as Plugin;
  return (plugin.transform as (code: string, id: string) => unknown)("", id);
}

describe("rejectUnsupported", () => {
  it("rejects an id that an extra entry matches", () => {
    const id = path.join(process.cwd(), "A.vue?vue&type=style");

    expect(() => transformOf([ALPHA], id)).toThrow(
      "ERR_INCA_TEST_ALPHA: A.vue uses an alpha block, unsupported for now: use a beta block",
    );
  });

  it("rejects a stylesheet import with no extra entries", () => {
    expect(() => transformOf([], "/app/x.scss")).toThrow("ERR_INCA_UNSUPPORTED_STYLESHEET");
  });

  it("prefers an extra entry over the stylesheet entry", () => {
    expect(() => transformOf([ALPHA], "/app/A.vue?type=style&lang.css")).toThrow(
      "ERR_INCA_TEST_ALPHA",
    );
  });

  it("lets every other id through", () => {
    expect(transformOf([ALPHA], "/app/main.ts")).toBeNull();
  });
});
