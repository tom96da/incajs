// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Runs one app under the real `inca-host` binary twice, as `inca dev
// --experimental-hmr` serves it and as a production build, and compares what
// the app prints about its encoding globals.

import { spawn } from "node:child_process";
import { access, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";

import { dedent } from "ts-dedent";
import { afterAll, beforeAll, expect, it, vi } from "vitest";

import { defaultBundler } from "../../packages/cli/src/defaultBundler.mts";
import { HostClient } from "../../packages/cli/src/dev-client/client.mts";
import { resolveTestHostBin } from "../support/hostBin.mts";
import { reloadHost, settleReloads } from "../support/reloads.mts";
import type { HmrChannel } from "../../packages/cli/src/types.mts";

/** Generous: a real process spawn, a real dev server and a real build sit in the loop. */
const WAIT_TIMEOUT_MS = 30_000;
/** How long the host is given to reload before this test gives up on it. */
const RELOAD_TIMEOUT_MS = 10_000;
/** What the app prints, decoded from the bytes of a hiragana letter. */
const EXPECTED = "[e2e] function function あ";

const ENTRY = dedent`
  console.log(
    "[e2e] " +
      typeof TextEncoder +
      " " +
      typeof TextDecoder +
      " " +
      new TextDecoder().decode(new Uint8Array([0xe3, 0x81, 0x82])),
  );
`;

/** The repository's built CLI runtime, which the development server loads in the host. */
const RUNTIME_DIST = path.resolve(import.meta.dirname, "../../packages/cli/dist/vite-runtime.js");

/** A writable that discards everything and silences the bundler's own logging. */
function discard(): Writable {
  return new Writable({
    write(_chunk, _encoding, callback) {
      callback();
    },
  });
}

/** The first complete line the app printed in `text`. */
function printed(text: string): string | undefined {
  return text
    .split("\n")
    .slice(0, -1)
    .find((line) => line.startsWith("[e2e] "));
}

const scratchRoot = path.join(import.meta.dirname, "tmp", "host-globals");
let appDir: string;
let entry: string;
let channel: HmrChannel | undefined;
let hostClient: HostClient | undefined;

beforeAll(async () => {
  await access(RUNTIME_DIST).catch(() => {
    throw new Error(
      `missing build output: ${RUNTIME_DIST}. run \`pnpm --filter @incajs/cli build\` first`,
    );
  });
  await mkdir(scratchRoot, { recursive: true });
  appDir = await mkdtemp(path.join(scratchRoot, "app-"));
  entry = path.join(appDir, "entry.mts");
  await writeFile(entry, ENTRY);
}, WAIT_TIMEOUT_MS);

afterAll(async () => {
  try {
    await channel?.close();
    await settleReloads();
  } finally {
    await hostClient?.stop();
    await rm(scratchRoot, { recursive: true, force: true });
  }
});

it(
  "prints the same encoding globals in development and in production",
  async () => {
    const outDir = path.join(appDir, "dist");
    await defaultBundler.build({
      entry,
      outDir,
      quiet: true,
      stdout: discard(),
      stderr: discard(),
    });
    const child = spawn(resolveTestHostBin(), [path.join(outDir, "bundle.js")], {
      stdio: ["ignore", "pipe", "pipe"],
    });
    let prodText = "";
    child.stdout.on("data", (chunk: Buffer) => (prodText += chunk.toString()));
    child.stderr.on("data", (chunk: Buffer) => (prodText += chunk.toString()));
    try {
      await vi.waitFor(() => expect(printed(prodText)).toBeDefined(), {
        timeout: WAIT_TIMEOUT_MS,
      });
    } finally {
      child.kill();
    }

    let devText = "";
    channel = await defaultBundler.hmr!({
      entry,
      cwd: appDir,
      stdout: discard(),
      stderr: discard(),
      quiet: true,
      notify: (payload) => hostClient?.notify("vite", payload),
      reload: () => reloadHost(hostClient, RELOAD_TIMEOUT_MS),
      onError: (error) => console.error(error.message),
    });
    hostClient = new HostClient({
      entryFile: channel.entryFile,
      hostBin: resolveTestHostBin(),
      onStderr: (line) => {
        devText += line;
      },
      integrations: { vite: (params) => channel?.dispatch(params) },
    });
    await hostClient.start();
    await vi.waitFor(() => expect(printed(devText)).toBeDefined(), { timeout: WAIT_TIMEOUT_MS });

    expect([printed(devText), printed(prodText)]).toEqual([EXPECTED, EXPECTED]);
  },
  WAIT_TIMEOUT_MS * 3,
);
