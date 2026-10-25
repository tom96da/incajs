// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { writeHmrEntry } from "../../src/vite/hmr.mts";

let cwd: string;

beforeAll(async () => {
  cwd = await mkdtemp(path.join(tmpdir(), "write-hmr-entry-"));
  await mkdir(path.join(cwd, "node_modules/.inca/hmr"), { recursive: true });
});

afterAll(async () => {
  await rm(cwd, { recursive: true, force: true });
});

/** The text of the entry written for an app at `/app/src/main.ts` with a width of 300. */
async function writtenEntry(): Promise<string> {
  const runtime = path.join(cwd, "dist/vite-runtime.js");
  const file = await writeHmrEntry(cwd, "/app/src/main.ts", { width: 300 }, runtime);
  return readFile(file, "utf8");
}

describe("writeHmrEntry", () => {
  it("awaits start, so the entry settles once the app has loaded", async () => {
    expect(await writtenEntry()).toMatch(/^await start\(/m);
  });

  it("passes the app's entry and the globals to start", async () => {
    expect(await writtenEntry()).toContain('start("/app/src/main.ts", {"width":300});');
  });

  it("imports start from the runtime by a relative path", async () => {
    expect(await writtenEntry()).toContain(
      'import { start } from "../../../dist/vite-runtime.js";',
    );
  });
});
