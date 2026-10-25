// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it } from "vitest";

import { withAppRoot } from "./appRoot.mts";

describe("withAppRoot", () => {
  it.each([
    ["outDir", "/app", "/app/dist", ".."],
    ["the same directory", "/app", "/app", "."],
    ["a nested directory", "/app", "/app/node_modules/.inca/hmr", "../../.."],
  ])("records the app's root relative to the config's directory (%s)", (_, root, dir, expected) => {
    expect(withAppRoot({}, root, dir)).toEqual({ root: expected });
  });

  it("keeps the members of the config", () => {
    expect(withAppRoot({ name: "Demo" }, "/app", "/app/dist")).toEqual({
      name: "Demo",
      root: "..",
    });
  });

  it.each([undefined, "text"])(
    "starts from an empty config for a value that is not an object (%j)",
    (config) => {
      expect(withAppRoot(config, "/app", "/app/dist")).toEqual({ root: ".." });
    },
  );
});
