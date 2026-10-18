// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Spawns the real `inca-host` binary: a unit test on `TestAppContext` never
// exercises process exit.

import { spawn } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { createInterface } from "node:readline";

import { expect, it } from "vitest";

import { resolveTestHostBin } from "../support/hostBin.mts";

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

it("a rejecting top-level await exits 1 after a report with a null id", async () => {
  const dir = await mkdtemp(path.join(tmpdir(), "inca-host-startup-"));
  await writeFile(
    path.join(dir, "bundle.js"),
    "await new Promise((_, reject) => { __inca_dev__.receive = () => reject(new Error('late')); });",
  );
  const child = spawn(resolveTestHostBin(), ["--dev", path.join(dir, "bundle.js")], {
    stdio: ["pipe", "pipe", "inherit"],
  });
  child.stdin.on("error", () => {});
  try {
    const lines: string[] = [];
    createInterface({ input: child.stdout }).on("line", (line) => lines.push(line));
    child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", method: "probe" })}\n`);
    const code = await new Promise<number | null>((resolve) => child.on("close", resolve));

    expect(code).toBe(1);
    const reply = JSON.parse(lines[0] ?? "null");
    expect(reply.id).toBeNull();
    expect(reply.error.code).toBe(-32000);
    expect(reply.error.message).toContain("late");
    expect(lines.some((line) => line.includes('"ready"'))).toBe(false);
  } finally {
    child.kill();
    await rm(dir, { recursive: true, force: true });
  }
}, 15_000);

/** Runs the host with `args` and collects its exit code and output. */
async function runHost(args: string[]) {
  const child = spawn(resolveTestHostBin(), args, { stdio: ["pipe", "pipe", "pipe"] });
  child.stdin.on("error", () => {});
  try {
    const out: string[] = [];
    let stderr = "";
    createInterface({ input: child.stdout }).on("line", (line) => out.push(line));
    child.stderr.on("data", (chunk) => (stderr += chunk));
    const code = await new Promise<number | null>((resolve) => child.on("close", resolve));
    return { code, out, stderr };
  } finally {
    child.kill();
  }
}

it("--dev without a path reports the missing bundle beside the executable", async () => {
  const { code, out, stderr } = await runHost(["--dev"]);

  expect(code).toBe(1);
  expect(out).toEqual([]);
  expect(stderr).toContain("no bundle.js found beside the executable");
  expect(stderr).not.toContain("failed to read");
}, 15_000);

it("an unreadable entry in development exits 1 after a report with a null id", async () => {
  const missing = path.join(tmpdir(), "inca-host-no-such-dir", "bundle.js");
  const { code, out } = await runHost(["--dev", missing]);

  expect(code).toBe(1);
  expect(out).toHaveLength(1);
  const reply = JSON.parse(out[0] ?? "null");
  expect(reply.id).toBeNull();
  expect(reply.error.code).toBe(-32000);
  expect(reply.error.message).toContain(`failed to read ${missing}`);
}, 15_000);

it("an unreadable entry in production prints to stderr and the host exits 1", async () => {
  const missing = path.join(tmpdir(), "inca-host-no-such-dir", "bundle.js");
  const { code, out, stderr } = await runHost([missing]);

  expect(code).toBe(1);
  expect(out).toEqual([]);
  expect(stderr).toContain(`failed to read ${missing}`);
}, 15_000);
