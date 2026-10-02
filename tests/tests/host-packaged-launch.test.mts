// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Launches the real `inca-host` the way a packaged app starts: no argv, a
// working directory unrelated to the app, and the bundle found beside the
// executable. The bundles create no window, so these cases need no display
// on any platform.

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { cp, link, mkdir, mkdtemp, realpath, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterEach, beforeEach, describe, expect, it } from "vitest";

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

let dir: string;

beforeEach(async () => {
  // The host reports the executable's resolved path, so compare against that.
  dir = await realpath(await mkdtemp(path.join(tmpdir(), "inca-host-packaged-")));
});

afterEach(async () => {
  await rm(dir, { recursive: true, force: true });
});

/** Puts the host at `exeDir/app`; hard-linked so `current_exe` stays in `exeDir`. */
async function placeExecutable(exeDir: string): Promise<string> {
  await mkdir(exeDir, { recursive: true });
  const exe = path.join(exeDir, "app");
  await link(hostBin, exe).catch(() => cp(hostBin, exe));
  return exe;
}

/** Runs `exe` with no argv from `/`, resolving with its exit code and stderr. */
function launch(exe: string): Promise<{ code: number | null; stdout: string; stderr: string }> {
  const env = { ...process.env };
  delete env.DISPLAY;
  delete env.WAYLAND_DISPLAY;
  const child = spawn(exe, [], { cwd: "/", env, stdio: ["ignore", "pipe", "pipe"] });
  let stdout = "";
  let stderr = "";
  child.stdout.setEncoding("utf8").on("data", (chunk: string) => (stdout += chunk));
  child.stderr.setEncoding("utf8").on("data", (chunk: string) => (stderr += chunk));
  return new Promise((resolve, reject) => {
    child.on("error", reject);
    child.on("close", (code) => resolve({ code, stdout, stderr }));
  });
}

const THROWING_BUNDLE = 'console.log("marker");\nthrow new Error("boom");\n';

describe("a packaged app launched with no arguments", () => {
  it(
    "runs the bundle beside the executable (flat layout)",
    async () => {
      const exe = await placeExecutable(path.join(dir, "app"));
      const bundle = path.join(dir, "app", "bundle.js");
      await writeFile(bundle, THROWING_BUNDLE);

      const { code, stdout, stderr } = await launch(exe);

      expect(code).toBe(1);
      expect(stdout).toBe("");
      expect(stderr).toMatch(/^marker\nError: boom\n/);
      expect(stderr).toContain(bundle);
    },
    TIMEOUT_MS,
  );

  it(
    "runs the bundle under Contents/Resources (.app layout)",
    async () => {
      const exe = await placeExecutable(path.join(dir, "App.app", "Contents", "MacOS"));
      const resources = path.join(dir, "App.app", "Contents", "Resources");
      await mkdir(resources, { recursive: true });
      await writeFile(path.join(resources, "bundle.js"), THROWING_BUNDLE);

      const { code, stdout, stderr } = await launch(exe);

      expect(code).toBe(1);
      expect(stdout).toBe("");
      expect(stderr).toMatch(/^marker\nError: boom\n/);
    },
    TIMEOUT_MS,
  );

  it(
    "exits 1 naming the missing bundle when none is beside the executable",
    async () => {
      const exe = await placeExecutable(path.join(dir, "app"));

      const { code, stdout, stderr } = await launch(exe);

      expect(code).toBe(1);
      expect(stdout).toBe("");
      expect(stderr).toContain("no bundle.js found beside the executable");
    },
    TIMEOUT_MS,
  );
});
