// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, symlink, writeFile } from "node:fs/promises";
import path from "node:path";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

import {
  resolveAppConfig,
  resolveBuildConfig,
  resolveRuntimeConfig,
} from "../src/config/loader.mts";
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

describe("resolveAppConfig's productName", () => {
  it.each(["My App", "日本語アプリ", "todo-2"])("keeps %j", async (productName) => {
    const app = await makeApp({ name: "scratch-app", inca: { productName } });

    await expect(resolveAppConfig(app)).resolves.toMatchObject({ productName });
  });

  it.each(["../escape", "a/b", "a\\b", "a:b", ".", "..", "   ", "My App ", "a\0b"])(
    "refuses %j, which can't name a directory",
    async (productName) => {
      const app = await makeApp({ name: "scratch-app", inca: { productName } });

      await expect(resolveAppConfig(app)).rejects.toThrow(
        expect.objectContaining({ code: "ERR_INCA_PRODUCT_NAME_INVALID" }),
      );
    },
  );

  it("refuses one that isn't a string", async () => {
    const app = await makeApp({ name: "scratch-app", inca: { productName: 1 } });

    await expect(resolveAppConfig(app)).rejects.toThrow(
      expect.objectContaining({ code: "ERR_INCA_PRODUCT_NAME_INVALID" }),
    );
  });
});

describe("resolveAppConfig's outDir", () => {
  it("resolves from the app's root", async () => {
    const app = await makeApp({ name: "scratch-app", inca: { outDir: "build" } });

    await expect(resolveAppConfig(app)).resolves.toMatchObject({
      outDir: path.join(app, "build"),
    });
  });

  it.each(["", ".", "..", "src", "node_modules"])(
    "refuses %j, which holds the app's sources",
    async (outDir) => {
      const app = await makeApp({ name: "scratch-app", inca: { outDir } });

      await expect(resolveAppConfig(app)).rejects.toThrow(
        expect.objectContaining({ code: "ERR_INCA_OUT_DIR_INVALID" }),
      );
    },
  );

  it("refuses one that holds a configured entry", async () => {
    const app = await makeApp({
      name: "scratch-app",
      inca: { outDir: "app", entry: "app/main.mts" },
    });

    await expect(resolveAppConfig(app)).rejects.toThrow(
      expect.objectContaining({ code: "ERR_INCA_OUT_DIR_INVALID" }),
    );
  });
});

describe("resolveBuildConfig's outDir", () => {
  it("refuses the app's root", async () => {
    const app = await makeApp({ name: "scratch-app", inca: { outDir: "." } });

    await expect(resolveBuildConfig(app)).rejects.toThrow(
      expect.objectContaining({ code: "ERR_INCA_OUT_DIR_INVALID" }),
    );
  });

  it.each(["dist", "..foo", "../sibling-out"])("allows %j", async (outDir) => {
    const app = await makeApp({ name: "scratch-app", inca: { outDir } });

    await expect(resolveBuildConfig(app)).resolves.toMatchObject({
      outDir: path.resolve(app, outDir),
    });
  });
});

describe.skipIf(process.platform === "win32")("outDir through a symlink", () => {
  it("refuses a symlink to src/", async () => {
    const app = await makeApp({ name: "scratch-app", inca: { outDir: "out" } });
    await mkdir(path.join(app, "src"));
    await symlink(path.join(app, "src"), path.join(app, "out"));

    await expect(resolveBuildConfig(app)).rejects.toThrow(
      expect.objectContaining({ code: "ERR_INCA_OUT_DIR_INVALID" }),
    );
  });

  it("refuses src/ reached through a symlink to the app's root", async () => {
    const app = await makeApp({ name: "scratch-app", inca: { outDir: "link/src" } });
    await symlink(app, path.join(app, "link"));

    await expect(resolveBuildConfig(app)).rejects.toThrow(
      expect.objectContaining({ code: "ERR_INCA_OUT_DIR_INVALID" }),
    );
  });

  it("refuses a dangling symlink into src/", async () => {
    const app = await makeApp({ name: "scratch-app", inca: { outDir: "out" } });
    await symlink(path.join(app, "src", "missing"), path.join(app, "out"));

    await expect(resolveBuildConfig(app)).rejects.toThrow(
      expect.objectContaining({ code: "ERR_INCA_OUT_DIR_INVALID" }),
    );
  });

  it("refuses a symlink to a directory that contains the app", async () => {
    const app = await makeApp({ name: "scratch-app", inca: { outDir: "up" } });
    await symlink(path.dirname(app), path.join(app, "up"));

    await expect(resolveBuildConfig(app)).rejects.toThrow(
      expect.objectContaining({ code: "ERR_INCA_OUT_DIR_INVALID" }),
    );
  });

  it("refuses an outDir in the real src/ when the app is reached through a symlink", async () => {
    const app = await makeApp({ name: "scratch-app" });
    await mkdir(path.join(app, "src"));
    const viaLink = `${app}-link`;
    await symlink(app, viaLink);
    await writeFile(
      path.join(app, "package.json"),
      JSON.stringify({
        type: "module",
        name: "scratch-app",
        inca: { outDir: path.join(app, "src") },
      }),
    );

    await expect(resolveBuildConfig(viaLink)).rejects.toThrow(
      expect.objectContaining({ code: "ERR_INCA_OUT_DIR_INVALID" }),
    );
  });

  it("allows a directory that does not exist yet", async () => {
    const app = await makeApp({ name: "scratch-app", inca: { outDir: "a/b/dist" } });

    await expect(resolveBuildConfig(app)).resolves.toMatchObject({
      outDir: path.join(app, "a/b/dist"),
    });
  });
});

describe("resolveRuntimeConfig", () => {
  it("refuses a name that can't name a directory", async () => {
    const app = await makeApp({ name: "scratch-app", inca: { productName: "a/b" } });

    await expect(resolveRuntimeConfig(app)).rejects.toThrow(
      expect.objectContaining({ code: "ERR_INCA_PRODUCT_NAME_INVALID" }),
    );
  });

  it("carries the window an app declared, and the name it is known by", async () => {
    const app = await makeApp({
      name: "scratch-app",
      inca: { productName: "Demo", window: { width: 1024 } },
    });

    await expect(resolveRuntimeConfig(app)).resolves.toEqual({
      name: "Demo",
      identifier: "org.inca.demo",
      window: { width: 1024 },
    });
  });

  it("resolves to nothing for an app that declares none of it", async () => {
    const app = await makeApp({});

    await expect(resolveRuntimeConfig(app)).resolves.toBeUndefined();
  });
});
