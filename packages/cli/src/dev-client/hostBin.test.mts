// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, describe, expect, it } from "vitest";

import { resolveHostBin } from "./hostBin.mts";

const ENV_VAR = "INCA_HOST_BIN";

describe("resolveHostBin", () => {
  afterEach(() => {
    delete process.env[ENV_VAR];
  });

  it("returns INCA_HOST_BIN unchanged when it's set", () => {
    process.env[ENV_VAR] = "/custom/path/to/inca-host";

    expect(resolveHostBin()).toBe("/custom/path/to/inca-host");
  });

  it("throws when neither an override nor a per-platform package resolves", () => {
    // This workspace's own @incajs/host-* packages carry no actual binary
    // (that's a CD-time concern), so resolution always falls through here.
    expect(() => resolveHostBin()).toThrow(new RegExp(ENV_VAR));
  });
});
