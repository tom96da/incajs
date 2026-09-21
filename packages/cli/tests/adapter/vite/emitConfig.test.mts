// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { readFile } from "node:fs/promises";
import path from "node:path";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { build } from "../../../src/adapter/vite/index.mts";
import { scratchApp } from "./scratchApp.mts";

const { setUp, tearDown, makeApp } = scratchApp("emit-config");

beforeAll(setUp);
afterAll(tearDown);

const APP = `<template><div>hi</div></template>\n`;

const CONFIG = {
  name: "Demo",
  identifier: "org.inca.demo",
  window: { width: 1024, height: 768, title: "Demo" },
};

describe("emitConfig", () => {
  it("writes the app's config beside the entry, and reports it as a build output", async () => {
    const { entry, outDir, streams } = await makeApp(APP);

    const output = await build({ entry, outDir, runtimeConfig: CONFIG, ...streams });

    expect(output.files).toContain("inca.json");
    const written = await readFile(path.join(outDir, "inca.json"), "utf8");
    expect(JSON.parse(written)).toEqual(CONFIG);
  }, 20000);

  it("writes nothing when the app declares no config", async () => {
    const { entry, outDir, streams } = await makeApp(APP);

    const output = await build({ entry, outDir, ...streams });

    expect(output.files).not.toContain("inca.json");
  }, 20000);

  it("writes nothing for a config with no keys", async () => {
    const { entry, outDir, streams } = await makeApp(APP);

    const output = await build({ entry, outDir, runtimeConfig: {}, ...streams });

    expect(output.files).not.toContain("inca.json");
  }, 20000);
});
