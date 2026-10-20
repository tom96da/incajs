// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Runs `inca config` against the real host binary, which prints and exits
// before any window opens.

import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { Writable } from "node:stream";

import { afterEach, beforeEach, expect, it } from "vitest";

import { printConfig } from "../../packages/cli/src/printConfig.mts";
import { resolveTestHostBin } from "../support/hostBin.mts";

const hostBin = resolveTestHostBin();

let dir: string;

beforeEach(async () => {
  dir = await mkdtemp(path.join(tmpdir(), "inca-config-"));
  await writeFile(path.join(dir, "package.json"), JSON.stringify({ type: "module" }));
});

afterEach(async () => {
  await rm(dir, { recursive: true, force: true });
});

/** A stream that appends what it is given to `append`. */
function sink(append: (text: string) => void): Writable {
  return new Writable({
    write(chunk, _encoding, callback) {
      append(String(chunk));
      callback();
    },
  });
}

/** Writes an `inca.config.ts` that declares `window`. */
async function declareWindow(window: object): Promise<void> {
  await writeFile(
    path.join(dir, "inca.config.ts"),
    `export default { window: ${JSON.stringify(window)} };\n`,
  );
}

it("prints the window width the app's config sets", async () => {
  await declareWindow({ width: 640 });
  let stdout = "";

  const code = await printConfig({
    cwd: dir,
    hostBin,
    stdout: sink((text) => (stdout += text)),
    stderr: sink(() => {}),
  });

  expect(code).toBe(0);
  expect(JSON.parse(stdout).window.width).toBe(640);
});

it("names the window setting the host cannot use", async () => {
  await declareWindow({ width: "wide" });

  await expect(
    printConfig({ cwd: dir, hostBin, stdout: sink(() => {}), stderr: sink(() => {}) }),
  ).rejects.toThrow(
    expect.objectContaining({
      code: "ERR_INCA_CONFIG_INVALID",
      message: 'window.width in inca.config.ts is "wide". it must be a number',
    }),
  );
});
