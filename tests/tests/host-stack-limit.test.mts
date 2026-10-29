// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Spawns the real `inca-host` binary: the stack limit of the engine sits inside the
// native stack of the main thread. The depth the script reaches depends on the platform, so the
// test reads only that the message carries one.

import { spawn } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { createInterface } from "node:readline";

import { expect, it } from "vitest";

import { resolveTestHostBin } from "../support/hostBin.mts";

/** Recurses until the stack limit throws, then reports how deep it got. */
const RECURSES_UNTIL_IT_THROWS = `
let depth = 0;
function recurse() {
  depth += 1;
  recurse();
}
try {
  recurse();
} catch {}
throw new Error(String(depth));
`;

it("the host survives recursion to the stack limit", async () => {
  const dir = await mkdtemp(path.join(tmpdir(), "inca-host-stack-"));
  await writeFile(path.join(dir, "bundle.js"), RECURSES_UNTIL_IT_THROWS);
  const child = spawn(resolveTestHostBin(), ["--dev", path.join(dir, "bundle.js")], {
    stdio: ["pipe", "pipe", "inherit"],
  });
  try {
    const lines: string[] = [];
    createInterface({ input: child.stdout }).on("line", (line) => lines.push(line));
    const [code, signal] = await new Promise<[number | null, NodeJS.Signals | null]>((resolve) =>
      child.on("close", (code, signal) => resolve([code, signal])),
    );

    expect(signal).toBeNull();
    expect(code).toBe(1);
    const reply = JSON.parse(lines[0] ?? "null");
    expect(reply.id).toBeNull();
    expect(reply.error.message).toMatch(/^Error: \d+$/);
  } finally {
    child.kill();
    await rm(dir, { recursive: true, force: true });
  }
}, 15_000);
