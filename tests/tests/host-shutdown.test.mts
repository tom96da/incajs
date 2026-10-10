// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Speaks the dev protocol to the real `inca-host` binary on raw stdio. On
// Linux the host runs on gpui's headless platform (the vitest setup removes
// the display variables), so no display is needed. On macOS the host opens a
// real window, which needs a window-server session.

import { spawn } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { createInterface } from "node:readline";
import type { ChildProcessWithoutNullStreams } from "node:child_process";

import { afterEach, beforeEach, expect, it, onTestFailed } from "vitest";

import { resolveTestHostBin } from "../support/hostBin.mts";

const TIMEOUT_MS = 15_000;

const hostBin = resolveTestHostBin();

let dir: string;
let child: ChildProcessWithoutNullStreams;
/** Every JSON line the host wrote to stdout, in order. */
let messages: Record<string, unknown>[];
let exited: Promise<number | null>;

beforeEach(async () => {
  dir = await mkdtemp(path.join(tmpdir(), "inca-host-shutdown-"));
  await writeFile(path.join(dir, "bundle.js"), "");
  child = spawn(hostBin, ["--dev", path.join(dir, "bundle.js")], {
    stdio: ["pipe", "pipe", "pipe"],
  });
  let stderr = "";
  child.stderr.setEncoding("utf8").on("data", (chunk: string) => (stderr += chunk));
  onTestFailed(() => console.error(`--- host stderr ---\n${stderr}`));
  // The host may exit before a late write lands.
  child.stdin.on("error", () => {});
  messages = [];
  createInterface({ input: child.stdout }).on("line", (line) => {
    try {
      messages.push(JSON.parse(line) as Record<string, unknown>);
    } catch {
      // A stray non-JSON line is not a protocol message.
    }
  });
  exited = new Promise((resolve) => child.on("close", resolve));
});

afterEach(async () => {
  child.kill();
  await exited;
  await rm(dir, { recursive: true, force: true });
});

/** Writes one request line. */
function send(message: object): void {
  child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", ...message })}\n`);
}

it(
  "answers shutdown with a null result under the same id after ready, then exits 0",
  async () => {
    send({ id: "s-1", method: "shutdown" });

    expect(await exited).toBe(0);
    expect(messages).toEqual([
      { jsonrpc: "2.0", method: "ready", params: { protocol: 0 } },
      { jsonrpc: "2.0", id: "s-1", result: null },
    ]);
  },
  TIMEOUT_MS,
);

it(
  "answers an unknown request -32601 under its id and keeps running",
  async () => {
    send({ id: 5, method: "nope" });
    send({ id: 6, method: "shutdown" });

    expect(await exited).toBe(0);
    expect(messages.slice(1)).toEqual([
      { jsonrpc: "2.0", id: 5, error: { code: -32601, message: "unknown method: nope" } },
      { jsonrpc: "2.0", id: 6, result: null },
    ]);
  },
  TIMEOUT_MS,
);

it(
  "leaves a request sent after shutdown unanswered",
  async () => {
    send({ id: 1, method: "shutdown" });
    send({ id: 2, method: "nope" });

    expect(await exited).toBe(0);
    expect(messages.map((m) => m.method ?? m.id)).toEqual(["ready", 1]);
  },
  TIMEOUT_MS,
);

it(
  "exits 0 when stdin closes, with nothing written after ready",
  async () => {
    child.stdin.end();

    expect(await exited).toBe(0);
    expect(messages).toEqual([{ jsonrpc: "2.0", method: "ready", params: { protocol: 0 } }]);
  },
  TIMEOUT_MS,
);
