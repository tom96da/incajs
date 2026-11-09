// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it } from "vitest";
import type { Plugin } from "vite";

import * as core from "../adapterCore.mts";
import { integrationPlugins } from "./integration.mts";
import type { ViteIntegration } from "./integration.mts";

const FRONTEND = ["alpha-pkg", "alpha"] as const;

const integration: ViteIntegration = {
  plugins: () => [{ name: "alpha-plugin" }],
  unsupported: [
    {
      code: "ERR_INCA_TEST_ALPHA",
      name: "an alpha file",
      matches: (id) => id.endsWith(".alpha"),
      hint: "use a beta file",
    },
  ],
  inject: {},
  styleModule: "alpha-style",
};

describe("integrationPlugins", () => {
  it("puts the plugins of the integration first", () => {
    const names = integrationPlugins(core, FRONTEND, integration, "/app").map(
      (plugin) => (plugin as Plugin).name,
    );

    expect(names).toEqual([
      "alpha-plugin",
      "inca:style-collect",
      "inca:style-apply",
      "inca:require-peers",
      "inca:reject-unsupported",
      "inca:inject",
    ]);
  });

  it("passes the unsupported entries of the integration to the check", () => {
    const check = integrationPlugins(core, FRONTEND, integration, "/app")[4] as Plugin;
    const transform = check.transform as (code: string, id: string) => unknown;

    expect(() => transform("", "/app/x.alpha")).toThrow("ERR_INCA_TEST_ALPHA");
  });
});
