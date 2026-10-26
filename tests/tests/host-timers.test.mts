// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Spawns the real `inca-host` binary: only a real process runs timers on the
// real clock.

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { cp, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterAll, expect, it, vi } from "vitest";

import { resolveTestHostBin } from "../support/hostBin.mts";

/** Generous: a real process spawn sits in the loop. */
const WAIT_TIMEOUT_MS = 30_000;

/** The built `incajs`. The test never builds it. */
const CORE_DIST = path.resolve(import.meta.dirname, "../../packages/core/dist");

if (!existsSync(CORE_DIST)) {
  throw new Error(`missing build output: ${CORE_DIST}. run \`pnpm --filter incajs build\` first`);
}

const BUNDLE = `import "./incajs/index.js";

let ticks = 0;
const id = setInterval(() => {
  ticks += 1;
  console.log("[e2e] tick " + ticks);
  if (ticks === 3) {
    clearInterval(id);
    setTimeout(() => console.log("[e2e] done"), 200);
  }
}, 20);
`;

let scratch: string | undefined;

afterAll(async () => {
  if (scratch) await rm(scratch, { recursive: true, force: true });
});

it(
  "stops an interval at clearInterval on the real clock",
  async () => {
    scratch = await mkdtemp(path.join(tmpdir(), "inca-host-timers-"));
    const bundle = path.join(scratch, "bundle.js");
    await writeFile(bundle, BUNDLE);
    await cp(CORE_DIST, path.join(scratch, "incajs"), { recursive: true });

    const child = spawn(resolveTestHostBin(), ["--dev", bundle], {
      stdio: ["pipe", "pipe", "pipe"],
    });
    child.stdin.on("error", () => {});
    let text = "";
    child.stdout.on("data", (chunk: Buffer) => (text += chunk.toString()));
    child.stderr.on("data", (chunk: Buffer) => (text += chunk.toString()));
    try {
      await vi.waitFor(() => expect(text).toContain("[e2e] done"), { timeout: WAIT_TIMEOUT_MS });

      expect(text.match(/\[e2e\] tick \d/g)).toEqual([
        "[e2e] tick 1",
        "[e2e] tick 2",
        "[e2e] tick 3",
      ]);
    } finally {
      child.kill();
    }
  },
  WAIT_TIMEOUT_MS + 10_000,
);
