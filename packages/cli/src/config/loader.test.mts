// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { resolveAppConfig, resolveBuildConfig, resolveRuntimeConfig } from "./loader.mts";

let cwd: string | undefined;

afterEach(async () => {
  if (cwd) await rm(cwd, { recursive: true, force: true });
  cwd = undefined;
});

async function makeApp(pkg: object): Promise<string> {
  cwd = await mkdtemp(path.join(tmpdir(), "inca-config-"));
  // Node needs "type": "module" to load an inca.config.ts without a
  // reparse-as-ESM warning. Every real app already has this (see
  // examples/*/package.json).
  await writeFile(path.join(cwd, "package.json"), JSON.stringify({ type: "module", ...pkg }));
  return cwd;
}

async function writeConfig(appCwd: string, body: string, ext = "ts"): Promise<void> {
  await writeFile(path.join(appCwd, `inca.config.${ext}`), body);
}

describe("resolveAppConfig", () => {
  it("derives productName, identifier and version from package.json alone", async () => {
    const app = await makeApp({ name: "click_counter", version: "0.0.0-0" });

    await expect(resolveAppConfig(app)).resolves.toMatchObject({
      productName: "click_counter",
      identifier: "org.inca.click-counter",
      identifierIsDefault: true,
      version: "0.0.0-0",
      icon: undefined,
      usedPackageJsonKey: false,
      outDir: path.join(app, "dist"),
    });
  });

  it("strips a package scope from the derived productName", async () => {
    const app = await makeApp({ name: "@my-org/notes-app", version: "1.0.0" });

    await expect(resolveAppConfig(app)).resolves.toMatchObject({
      productName: "notes-app",
      identifier: "org.inca.notes-app",
    });
  });

  it('lets a package.json "inca" key override productName and identifier', async () => {
    const app = await makeApp({
      name: "click_counter",
      version: "0.0.0-0",
      inca: { productName: "Click Counter", identifier: "com.example.click-counter" },
    });

    await expect(resolveAppConfig(app)).resolves.toMatchObject({
      productName: "Click Counter",
      identifier: "com.example.click-counter",
      identifierIsDefault: false,
    });
  });

  it('flags a package.json "inca" key as deprecated', async () => {
    const app = await makeApp({
      name: "click_counter",
      version: "0.0.0-0",
      inca: { productName: "Click Counter", identifier: "com.example.click-counter" },
    });

    await expect(resolveAppConfig(app)).resolves.toMatchObject({ usedPackageJsonKey: true });
  });

  it("lets inca.config.ts override productName and identifier", async () => {
    const app = await makeApp({ name: "click_counter", version: "0.0.0-0" });
    await writeConfig(
      app,
      `export default { productName: "Click Counter", identifier: "com.example.click-counter" };\n`,
    );

    await expect(resolveAppConfig(app)).resolves.toMatchObject({
      productName: "Click Counter",
      identifier: "com.example.click-counter",
      identifierIsDefault: false,
      usedPackageJsonKey: false,
    });
  });

  it('prefers inca.config.ts over a package.json "inca" key set at the same time', async () => {
    const app = await makeApp({
      name: "click_counter",
      version: "0.0.0-0",
      inca: { productName: "From package.json" },
    });
    await writeConfig(app, `export default { productName: "From config" };\n`);

    const config = await resolveAppConfig(app);
    expect(config.productName).toBe("From config");
    expect(config.usedPackageJsonKey).toBe(true);
  });

  it("resolves a relative entry and outDir against cwd", async () => {
    const app = await makeApp({ name: "click_counter", version: "0.0.0-0" });
    await writeConfig(app, `export default { entry: "./src/main.mts", outDir: "out" };\n`);

    await expect(resolveAppConfig(app)).resolves.toMatchObject({
      entry: path.join(app, "src/main.mts"),
      outDir: path.join(app, "out"),
    });
  });

  it("reads sourcemap: true from inca.config.ts", async () => {
    const app = await makeApp({ name: "click_counter", version: "0.0.0-0" });
    await writeConfig(app, `export default { sourcemap: true };\n`);

    await expect(resolveAppConfig(app)).resolves.toMatchObject({ sourcemap: true });
  });

  it("loads inca.config.js and inca.config.json too", async () => {
    const jsApp = await makeApp({ name: "click_counter", version: "0.0.0-0" });
    await writeConfig(jsApp, `export default { productName: "From JS" };\n`, "js");
    await expect(resolveAppConfig(jsApp)).resolves.toMatchObject({ productName: "From JS" });

    const jsonApp = await makeApp({ name: "click_counter", version: "0.0.0-0" });
    await writeConfig(jsonApp, JSON.stringify({ productName: "From JSON" }), "json");
    await expect(resolveAppConfig(jsonApp)).resolves.toMatchObject({ productName: "From JSON" });
  });

  it('resolves "inca".icon relative to cwd and rejects a missing one', async () => {
    const app = await makeApp({
      name: "click_counter",
      version: "0.0.0-0",
      inca: { icon: "assets/icon.icns" },
    });

    await expect(resolveAppConfig(app)).rejects.toThrow(/icon\.icns/);
  });

  it("throws when package.json is missing", async () => {
    cwd = await mkdtemp(path.join(tmpdir(), "inca-config-"));

    await expect(resolveAppConfig(cwd)).rejects.toThrow(/package\.json/);
  });

  it("throws when package.json has no name and no productName override", async () => {
    const app = await makeApp({ version: "0.0.0-0" });

    await expect(resolveAppConfig(app)).rejects.toThrow(/productName/);
  });

  it("throws one coded error naming the file when package.json is malformed", async () => {
    const app = await makeApp({});
    await writeFile(path.join(app, "package.json"), "{ not json");

    await expect(resolveAppConfig(app)).rejects.toThrow(
      expect.objectContaining({
        code: "ERR_INCA_PACKAGE_JSON_INVALID",
        message: expect.stringContaining(path.join(app, "package.json")),
      }),
    );
  });

  // c12 adds the `jiti` hint only when `jiti` is not installed, and none is in node_modules today.
  describe("a config file that fails to load", () => {
    async function loadFailure(body: string): Promise<Error> {
      const app = await makeApp({ name: "click_counter", version: "0.0.0-0" });
      await writeConfig(app, body);
      return resolveAppConfig(app).then(
        () => {
          throw new Error("expected a rejection");
        },
        (error: Error) => error,
      );
    }

    it("rejects with the first error's message and the file name, without jiti", async () => {
      const error = await loadFailure("export default {;\n");

      expect(error.message).toContain("inca.config.ts");
      expect(error.message).toContain((error.cause as Error).message);
      expect(error.message).not.toContain("jiti");
    });

    it("keeps the first error as the cause", async () => {
      const error = await loadFailure("export default {;\n");

      expect(error.cause).toBeInstanceOf(Error);
    });

    it("has no jiti in the stack", async () => {
      const error = await loadFailure("export default {;\n");

      expect(error.stack).not.toContain("jiti");
    });

    it("rethrows an error from a default export function unchanged", async () => {
      const error = await loadFailure(
        `export default () => { throw Object.assign(new Error("boom"), { code: "E_BOOM" }); };\n`,
      );

      expect(error).toMatchObject({ message: "boom", code: "E_BOOM" });
    });
  });
});

describe("dev.hmr", () => {
  it("is on for an app that declares no dev", async () => {
    const app = await makeApp({ name: "click_counter" });

    await expect(resolveBuildConfig(app)).resolves.toMatchObject({ dev: { hmr: true } });
  });

  it("is what the app declared, for a build config and an app config", async () => {
    const app = await makeApp({ name: "click_counter", version: "0.0.0-0" });
    await writeConfig(app, `export default { dev: { hmr: false } };\n`);

    await expect(resolveBuildConfig(app)).resolves.toMatchObject({ dev: { hmr: false } });
    await expect(resolveAppConfig(app)).resolves.toMatchObject({ dev: { hmr: false } });
  });

  describe.each([
    ["resolveBuildConfig", resolveBuildConfig],
    ["resolveRuntimeConfig", resolveRuntimeConfig],
  ])("%s refuses", (_, resolve) => {
    it.each([
      [`"false"`, `"false"`],
      ["0", "0"],
      [`"yes"`, `"yes"`],
      ["null", "null"],
      ["() => 1", "() => 1"],
    ])("a dev.hmr of %s", async (value, shown) => {
      const app = await makeApp({ name: "click_counter" });
      await writeConfig(app, `export default { dev: { hmr: ${value} } };\n`);

      await expect(resolve(app)).rejects.toThrow(
        expect.objectContaining({
          code: "ERR_INCA_CONFIG_INVALID",
          message: expect.stringContaining(
            `dev.hmr in inca.config.ts is ${shown}. it must be true or false`,
          ),
        }),
      );
    });

    it.each([`"x"`, "[]"])("a dev of %s", async (value) => {
      const app = await makeApp({ name: "click_counter" });
      await writeConfig(app, `export default { dev: ${value} };\n`);

      await expect(resolve(app)).rejects.toThrow(
        expect.objectContaining({
          code: "ERR_INCA_CONFIG_INVALID",
          message: expect.stringContaining(
            `dev in inca.config.ts is ${value}. it must be an object`,
          ),
        }),
      );
    });
  });
});
