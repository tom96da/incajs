// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { resolveAppConfig } from "./loader.mts";

let cwd: string | undefined;

afterEach(async () => {
  if (cwd) await rm(cwd, { recursive: true, force: true });
  cwd = undefined;
});

async function makeApp(pkg: object): Promise<string> {
  cwd = await mkdtemp(path.join(tmpdir(), "inca-config-"));
  // Node needs "type": "module" to load an inca.config.ts without a
  // reparse-as-ESM warning — every real app already has this (see
  // examples/*/package.json), so this just matches that baseline.
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

  it('lets a package.json "inca" key override productName and identifier, and flags it as deprecated', async () => {
    const app = await makeApp({
      name: "click_counter",
      version: "0.0.0-0",
      inca: { productName: "Click Counter", identifier: "com.example.click-counter" },
    });

    await expect(resolveAppConfig(app)).resolves.toMatchObject({
      productName: "Click Counter",
      identifier: "com.example.click-counter",
      identifierIsDefault: false,
      usedPackageJsonKey: true,
    });
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
});
