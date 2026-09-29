// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Spawns the real `inca-host` binary: a unit test on `TestAppContext` never
// exercises process exit.

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { createInterface } from "node:readline";

import { expect, it } from "vitest";

/** `target/debug/inca-host` or `target/release/inca-host`; never built here. */
function resolveTestHostBin(): string {
  const repoRoot = path.resolve(import.meta.dirname, "../..");
  const debug = path.join(repoRoot, "target/debug/inca-host");
  const release = path.join(repoRoot, "target/release/inca-host");
  if (existsSync(debug)) return debug;
  if (existsSync(release)) return release;
  throw new Error(
    `no inca-host binary found at ${debug} or ${release} — run \`cargo build -p inca-host\` first`,
  );
}

it("a throwing first bundle is reported on stdout and the host exits 1", async () => {
  const dir = await mkdtemp(path.join(tmpdir(), "inca-host-startup-"));
  await writeFile(path.join(dir, "bundle.js"), "throw new Error('boom');");
  const child = spawn(resolveTestHostBin(), ["--dev", path.join(dir, "bundle.js")], {
    stdio: ["pipe", "pipe", "inherit"],
  });
  try {
    const lines: string[] = [];
    createInterface({ input: child.stdout }).on("line", (line) => lines.push(line));
    const code = await new Promise<number | null>((resolve) => child.on("close", resolve));

    expect(code).toBe(1);
    const reply = JSON.parse(lines[0] ?? "null");
    expect(reply.id).toBeNull();
    expect(reply.error.code).toBe(-32000);
  } finally {
    child.kill();
    await rm(dir, { recursive: true, force: true });
  }
}, 15_000);
