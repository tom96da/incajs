// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Spawns the real `inca-host` binary on a development build: the source-map
// lookup runs in the host, so only a real process reaches it.

import { spawn } from "node:child_process";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { Writable } from "node:stream";

import { afterAll, expect, it } from "vitest";

import { defaultBundler } from "../../packages/cli/src/defaultBundler.mts";
import { resolveTestHostBin } from "../support/hostBin.mts";
import type { BuildOutput, Watcher } from "../../packages/cli/src/adapter/types.mts";

/** The throw sits on line 5, after an interface that the generated code drops. */
const SOURCE = [
  "interface Boom {",
  "  value: number;",
  "}",
  "const boom: Boom = { value: 1 };",
  'throw new Error("mapped boom");',
  "",
].join("\n");

let scratch: string | undefined;

afterAll(async () => {
  if (scratch) await rm(scratch, { recursive: true, force: true });
});

/** A writable that discards everything and silences the bundler's own output. */
function discard(): Writable {
  return new Writable({
    write(_chunk, _encoding, callback) {
      callback();
    },
  });
}

it("reports a thrown entry at its source file, line and column", async () => {
  scratch = await mkdtemp(path.join(tmpdir(), "inca-host-source-map-"));
  const entry = path.join(scratch, "app/main.ts");
  const outDir = path.join(scratch, "dist");
  await mkdir(path.dirname(entry), { recursive: true });
  await writeFile(entry, SOURCE);

  let resolveBuild!: (output: BuildOutput) => void;
  let rejectBuild!: (error: Error) => void;
  const built = new Promise<BuildOutput>((resolve, reject) => {
    resolveBuild = resolve;
    rejectBuild = reject;
  });
  const watcher: Watcher = await defaultBundler.watch({
    entry,
    outDir,
    mode: "development",
    quiet: true,
    stdout: discard(),
    stderr: discard(),
    onBuild: resolveBuild,
    onError: (failure) => rejectBuild(new Error(failure.message)),
  });
  try {
    await built;
  } finally {
    await watcher.close();
  }

  const child = spawn(resolveTestHostBin(), ["--dev", path.join(outDir, "bundle.js")], {
    stdio: ["pipe", "pipe", "pipe"],
  });
  child.stdin.on("error", () => {});
  let text = "";
  child.stdout.on("data", (chunk: Buffer) => (text += chunk.toString()));
  child.stderr.on("data", (chunk: Buffer) => (text += chunk.toString()));
  try {
    const code = await new Promise<number | null>((resolve) => child.on("close", resolve));

    expect(code).toBe(1);
    expect(text).toContain("main.ts:5:11");
    expect(text).not.toContain("bundle.js:");
  } finally {
    child.kill();
  }
}, 60_000);
