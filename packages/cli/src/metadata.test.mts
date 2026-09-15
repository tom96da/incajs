// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { readAppMetadata } from "./metadata.mts";

let cwd: string | undefined;

afterEach(async () => {
  if (cwd) await rm(cwd, { recursive: true, force: true });
  cwd = undefined;
});

async function makeApp(pkg: object): Promise<string> {
  cwd = await mkdtemp(path.join(tmpdir(), "inca-metadata-"));
  await writeFile(path.join(cwd, "package.json"), JSON.stringify(pkg));
  return cwd;
}

describe("readAppMetadata", () => {
  it("derives productName, identifier and version from package.json alone", async () => {
    const app = await makeApp({ name: "click_counter", version: "0.0.0-0" });

    await expect(readAppMetadata(app)).resolves.toEqual({
      productName: "click_counter",
      identifier: "org.inca.click-counter",
      identifierIsDefault: true,
      version: "0.0.0-0",
      icon: undefined,
    });
  });

  it("strips a package scope from the derived productName", async () => {
    const app = await makeApp({ name: "@my-org/notes-app", version: "1.0.0" });

    await expect(readAppMetadata(app)).resolves.toMatchObject({
      productName: "notes-app",
      identifier: "org.inca.notes-app",
    });
  });

  it('lets a "inca" key override productName and identifier', async () => {
    const app = await makeApp({
      name: "click_counter",
      version: "0.0.0-0",
      inca: { productName: "Click Counter", identifier: "com.example.click-counter" },
    });

    await expect(readAppMetadata(app)).resolves.toMatchObject({
      productName: "Click Counter",
      identifier: "com.example.click-counter",
      identifierIsDefault: false,
    });
  });

  it("resolves inca.icon relative to cwd and rejects a missing one", async () => {
    const app = await makeApp({
      name: "click_counter",
      version: "0.0.0-0",
      inca: { icon: "assets/icon.icns" },
    });

    await expect(readAppMetadata(app)).rejects.toThrow(/icon\.icns/);
  });

  it("throws when package.json is missing", async () => {
    cwd = await mkdtemp(path.join(tmpdir(), "inca-metadata-"));

    await expect(readAppMetadata(cwd)).rejects.toThrow(/package\.json/);
  });

  it("throws when package.json has no name and no productName override", async () => {
    const app = await makeApp({ version: "0.0.0-0" });

    await expect(readAppMetadata(app)).rejects.toThrow(/productName/);
  });
});
