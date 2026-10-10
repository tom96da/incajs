// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Spawns the real `inca-host` binary on entries whose top-level `await` stays
// pending, to check when the window opens and `ready` is sent. Linux runs the
// host headless (the vitest setup removes the display variables). macOS opens
// a real window.

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { createInterface } from "node:readline";

import { expect, it, vi } from "vitest";
import type { TestContext } from "vitest";

const TIMEOUT_MS = 15_000;

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

const hostBin = resolveTestHostBin();

interface Started {
  /** Every JSON line the host wrote to stdout, in order. */
  messages: Record<string, unknown>[];
  exited: Promise<number | null>;
  send: (message: object) => void;
}

/**
 * Starts the host with `--dev` on an entry holding `source`. The test context
 * keeps each test's host and directory apart, so the tests can run together.
 */
async function startHost(
  source: string,
  { onTestFailed, onTestFinished }: TestContext,
): Promise<Started> {
  const dir = await mkdtemp(path.join(tmpdir(), "inca-host-wait-"));
  await writeFile(path.join(dir, "bundle.js"), source);
  const spawned = spawn(hostBin, ["--dev", path.join(dir, "bundle.js")], {
    stdio: ["pipe", "pipe", "pipe"],
  });
  let stderr = "";
  spawned.stderr.setEncoding("utf8").on("data", (chunk: string) => (stderr += chunk));
  onTestFailed(() => console.error(`--- host stderr ---\n${stderr}`));
  // The host may exit before a late write lands.
  spawned.stdin.on("error", () => {});
  const messages: Record<string, unknown>[] = [];
  createInterface({ input: spawned.stdout }).on("line", (line) => {
    try {
      messages.push(JSON.parse(line) as Record<string, unknown>);
    } catch {
      // A stray non-JSON line is not a protocol message.
    }
  });
  const exited = new Promise<number | null>((resolve) => spawned.on("close", resolve));
  onTestFinished(async () => {
    spawned.kill();
    await exited;
    await rm(dir, { recursive: true, force: true });
  });
  return {
    messages,
    exited,
    send: (message) => {
      spawned.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", ...message })}\n`);
    },
  };
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

it.concurrent(
  "answers shutdown while the entry is pending, with no ready",
  async (ctx) => {
    const host = await startHost("await new Promise(() => {});", ctx);
    host.send({ id: 1, method: "shutdown" });

    expect(await host.exited).toBe(0);
    expect(host.messages).toEqual([{ jsonrpc: "2.0", id: 1, result: null }]);
  },
  TIMEOUT_MS,
);

it.concurrent(
  "opens the window and sends ready after the timeout when the entry never settles",
  async (ctx) => {
    const started = Date.now();
    const host = await startHost("await new Promise(() => {});", ctx);
    await sleep(1000);
    expect(host.messages).toEqual([]);

    await vi.waitFor(() => expect(host.messages).toHaveLength(1), { timeout: 6000 });
    expect(Date.now() - started).toBeGreaterThanOrEqual(1500);
    expect(host.messages[0]).toEqual({ jsonrpc: "2.0", method: "ready", params: { protocol: 0 } });

    host.send({ id: 2, method: "shutdown" });
    expect(await host.exited).toBe(0);
  },
  TIMEOUT_MS,
);

it.concurrent(
  "keeps waiting with no window while the app reports a failed load",
  async (ctx) => {
    const host = await startHost(
      "__inca_dev__.setLoadFailed(true); await new Promise(() => {});",
      ctx,
    );
    await sleep(2500);
    expect(host.messages).toEqual([]);

    host.send({ id: 3, method: "shutdown" });
    expect(await host.exited).toBe(0);
    expect(host.messages).toEqual([{ jsonrpc: "2.0", id: 3, result: null }]);
  },
  TIMEOUT_MS,
);
