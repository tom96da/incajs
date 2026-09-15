// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, describe, expect, it } from "vitest";

import { resolveHostBin } from "./hostBin.mts";

const ENV_VAR = "GPJS_UI_HOST_BIN";

describe("resolveHostBin", () => {
  afterEach(() => {
    delete process.env[ENV_VAR];
  });

  it("returns GPJS_UI_HOST_BIN unchanged when it's set", () => {
    process.env[ENV_VAR] = "/custom/path/to/gpjs-ui-host";

    expect(resolveHostBin()).toBe("/custom/path/to/gpjs-ui-host");
  });

  it("otherwise resolves inside this workspace's own Cargo build output", () => {
    // No filesystem access here — this only checks the path shape, so it
    // holds regardless of whether that binary has actually been built. It
    // also relies on the per-platform packages carrying no actual binary in
    // this workspace's own node_modules, so resolution always falls through.
    expect(resolveHostBin()).toMatch(/[/\\]target[/\\]debug[/\\]gpjs-ui-host(\.exe)?$/);
  });

  it("falls back into the release profile when asked", () => {
    expect(resolveHostBin({ profile: "release" })).toMatch(
      /[/\\]target[/\\]release[/\\]gpjs-ui-host(\.exe)?$/,
    );
  });
});
