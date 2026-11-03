// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { access, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { entryRoot, resolveEntry, selectEntry } from "./entry.mts";
import type { Frontend } from "./types.mts";

const ALPHA: Frontend = {
  id: ["alpha-pkg", "alpha"],
  appFile: "App.alpha",
  entryCode: (appImport) => `load(${appImport});`,
};

let cwd: string | undefined;

afterEach(async () => {
  if (cwd) await rm(cwd, { recursive: true, force: true });
  cwd = undefined;
});

async function makeApp(): Promise<string> {
  cwd = await mkdtemp(path.join(tmpdir(), "inca-entry-"));
  await mkdir(path.join(cwd, "src"), { recursive: true });
  return cwd;
}

describe("resolveEntry", () => {
  it("writes the entry text of the frontend for its app file", async () => {
    const app = await makeApp();
    const appPath = path.join(app, "src/App.alpha");
    await writeFile(appPath, "alpha\n");

    const entry = await resolveEntry(app, ALPHA);

    expect(entry).toBe(path.join(app, "node_modules/.inca/entry.mts"));
    expect(await readFile(entry, "utf8")).toBe(
      `load(${JSON.stringify(appPath.split(path.sep).join("/"))});`,
    );
  });

  it("ignores an App.vue the frontend does not name", async () => {
    const app = await makeApp();
    await writeFile(path.join(app, "src/App.vue"), "<template><div/></template>\n");

    await expect(resolveEntry(app, ALPHA)).rejects.toMatchObject({
      code: "ERR_INCA_ENTRY_NOT_FOUND",
    });
  });

  it("prefers a committed src/main.mts over src/App.vue when both exist", async () => {
    const app = await makeApp();
    const mainPath = path.join(app, "src/main.mts");
    await writeFile(mainPath, "// a hand-written entry\n");
    await writeFile(path.join(app, "src/App.vue"), "<template><div/></template>\n");

    await expect(resolveEntry(app, ALPHA)).resolves.toBe(mainPath);
  });

  it.each(["main.ts", "main.js"])("uses a committed src/%s", async (file) => {
    const app = await makeApp();
    const mainPath = path.join(app, "src", file);
    await writeFile(mainPath, "// a hand-written entry\n");
    await writeFile(path.join(app, "src/App.vue"), "<template><div/></template>\n");

    await expect(resolveEntry(app, ALPHA)).resolves.toBe(mainPath);
  });

  it("prefers main.mts over main.ts over main.js", async () => {
    const app = await makeApp();
    await writeFile(path.join(app, "src/main.js"), "");
    await writeFile(path.join(app, "src/main.ts"), "");
    await expect(resolveEntry(app, ALPHA)).resolves.toBe(path.join(app, "src/main.ts"));

    await writeFile(path.join(app, "src/main.mts"), "");
    await expect(resolveEntry(app, ALPHA)).resolves.toBe(path.join(app, "src/main.mts"));
  });

  it("names the app file of the frontend in the missing-entry error", async () => {
    const app = await makeApp();

    await expect(resolveEntry(app, ALPHA)).rejects.toThrow(
      `no app entry found. check that ${path.join(app, "src/main.ts")} or ${path.join(app, "src/App.alpha")} exists`,
    );
  });
});

describe("selectEntry", () => {
  it("gives the synthesized entry path for the app file of the frontend", async () => {
    const app = await makeApp();
    await writeFile(path.join(app, "src/App.alpha"), "alpha\n");

    expect(selectEntry(app, ALPHA)).toBe(path.join(app, "node_modules/.inca/entry.mts"));
    await expect(access(path.join(app, "node_modules/.inca/entry.mts"))).rejects.toMatchObject({
      code: "ENOENT",
    });
  });
});

describe("entryRoot", () => {
  it("gives src for the entry synthesized for App.vue", () => {
    const app = path.join(tmpdir(), "inca-app");

    expect(entryRoot(app, path.join(app, "node_modules/.inca/entry.mts"))).toBe(
      path.join(app, "src"),
    );
  });

  it("gives the directory of any other entry", () => {
    const app = path.join(tmpdir(), "inca-app");

    expect(entryRoot(app, path.join(app, "app/main.ts"))).toBe(path.join(app, "app"));
  });
});
