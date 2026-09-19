// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { writeFile } from "node:fs/promises";
import path from "node:path";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { resolveAppConfig } from "../src/config/loader.mts";
import { scratchConfigApp } from "./scratchConfigApp.mts";

const { setUp, tearDown, makeApp } = scratchConfigApp("config");

beforeAll(setUp);
afterAll(tearDown);

describe("resolveAppConfig against a real inca.config.ts", () => {
  it("loads a config file that itself imports defineConfig", async () => {
    const app = await makeApp({ name: "scratch-app" });

    const definePath = path
      .relative(app, path.join(import.meta.dirname, "../src/config/index.mts"))
      .split(path.sep)
      .join("/");
    await writeFile(
      path.join(app, "inca.config.ts"),
      `import { defineConfig } from "${definePath}";\n\n` +
        `export default defineConfig({ productName: "Scratch App" });\n`,
    );

    await expect(resolveAppConfig(app)).resolves.toMatchObject({
      productName: "Scratch App",
    });
  });
});
