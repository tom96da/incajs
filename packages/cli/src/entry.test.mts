// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { resolveEntry } from "./entry.mts";

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
  it("wraps src/App.vue in a synthesized entry when there's no main.mts", async () => {
    const app = await makeApp();
    const appVuePath = path.join(app, "src/App.vue");
    await writeFile(appVuePath, "<template><div/></template>\n");

    const entry = await resolveEntry(app);

    expect(entry).toBe(path.join(app, "node_modules/.inca/entry.mts"));
    const content = await readFile(entry, "utf8");
    expect(content).toContain('import { createIncaApp } from "incajs/vue";');
    expect(content).toContain(
      `import App from ${JSON.stringify(appVuePath.split(path.sep).join("/"))};`,
    );
    expect(content).toContain("createIncaApp(App).mount();");
  });

  it("prefers a committed src/main.mts over src/App.vue when both exist", async () => {
    const app = await makeApp();
    const mainPath = path.join(app, "src/main.mts");
    await writeFile(mainPath, "// a hand-written entry\n");
    await writeFile(path.join(app, "src/App.vue"), "<template><div/></template>\n");

    await expect(resolveEntry(app)).resolves.toBe(mainPath);
  });

  it("throws naming both filenames when neither exists", async () => {
    const app = await makeApp();

    await expect(resolveEntry(app)).rejects.toThrow(/expected .*main\.mts.* or .*App\.vue/);
  });
});
