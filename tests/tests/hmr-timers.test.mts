// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Drives the real `inca-host` binary and a real Vite dev server through a
// script edit, and counts the ticks of the timer that the old module started.

import { existsSync } from "node:fs";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";

import { afterAll, beforeAll, expect, it, onTestFailed, vi } from "vitest";

import { defaultBundler } from "../../packages/cli/src/defaultBundler.mts";
import { HostClient } from "../../packages/cli/src/dev-client/hostClient.mts";
import { resolveTestHostBin } from "../support/hostBin.mts";
import { reloadHost, settleReloads } from "../support/reloads.mts";
import type { HmrChannel } from "../../packages/cli/src/types.mts";

/** How long the host is given to reload/relay before this test gives up on it. */
const RELOAD_TIMEOUT_MS = 10_000;
/** Generous: a real process spawn, a real dev server, and a real file watcher all sit in the loop. */
const WAIT_TIMEOUT_MS = 30_000;

/** A writable that discards everything and silences the dev server's own logging. */
function discard(): Writable {
  return new Writable({
    write(_chunk, _encoding, callback) {
      callback();
    },
  });
}

/**
 * `incajs`'s and `@incajs/cli`'s built `dist/`. The fixture's entry
 * imports both. This test never builds them and fails with a message when
 * they are missing.
 */
function checkJsPackagesAreBuilt(): void {
  const repoRoot = path.resolve(import.meta.dirname, "../..");
  const missing = [
    path.join(repoRoot, "packages/core/dist"),
    path.join(repoRoot, "packages/cli/dist/vite-runtime.js"),
  ].filter((p) => !existsSync(p));
  if (missing.length > 0) {
    throw new Error(
      `missing build output: ${missing.join(", ")}. run ` +
        "`pnpm --filter incajs --filter @incajs/cli build` first",
    );
  }
}

const hostBin = resolveTestHostBin();
checkJsPackagesAreBuilt();

const ENTRY_MTS = `
import { createIncaApp } from "incajs/vue";
import App from "./App.vue";

createIncaApp(App).mount();
`;

function appVue(generation: number): string {
  return `<script>
const GENERATION = ${generation};
console.log("[e2e] evaluated gen=" + GENERATION);
setInterval(() => console.log("[e2e] timer gen=" + GENERATION), 50);
export default {};
</script>

<template>
  <div>timer</div>
</template>
`;
}

const scratchRoot = path.join(import.meta.dirname, "tmp", "hmr-timers");
let fixtureDir: string;
let vuePath: string;
let channel: HmrChannel | undefined;
let hostClient: HostClient | undefined;
let stderrText = "";

beforeAll(async () => {
  await mkdir(scratchRoot, { recursive: true });
  fixtureDir = await mkdtemp(path.join(scratchRoot, "app-"));
  vuePath = path.join(fixtureDir, "App.vue");
  const entry = path.join(fixtureDir, "entry.mts");
  await writeFile(vuePath, appVue(1));
  await writeFile(entry, ENTRY_MTS);

  channel = await defaultBundler.hmr!({
    entry,
    cwd: fixtureDir,
    stdout: discard(),
    stderr: discard(),
    quiet: true,
    notify: (payload) => hostClient?.notify("vite", payload),
    // A full reload needs a fresh Session, which only inca-host's own
    // `reload` RPC gives it. A Vue SFC edit always has an HMR boundary, so it
    // is not expected to fire here. It is wired for real just in case.
    reload: () => reloadHost(hostClient, RELOAD_TIMEOUT_MS),
    onError: (error) => console.error(error.message),
  });

  hostClient = new HostClient({
    entryFile: channel.entryFile,
    hostBin,
    onStderr: (line) => {
      stderrText += line;
    },
    integrations: {
      vite: (params) => channel?.dispatch(params),
    },
  });
  await hostClient.start();
}, WAIT_TIMEOUT_MS + 10_000);

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
  "keeps the old timer ticking after a script edit",
  async () => {
    onTestFailed(() => console.error(`--- host stderr ---\n${stderrText}`));

    await vi.waitFor(() => expect(stderrText).toContain("[e2e] timer gen=1"), {
      timeout: WAIT_TIMEOUT_MS,
    });
    await writeFile(vuePath, appVue(2));
    await vi.waitFor(() => expect(stderrText).toContain("[e2e] evaluated gen=2"), {
      timeout: WAIT_TIMEOUT_MS,
    });
    const oldTicks = () =>
      stderrText.slice(stderrText.indexOf("[e2e] evaluated gen=2")).match(/\[e2e\] timer gen=1/g)
        ?.length ?? 0;
    await vi.waitFor(() => expect(oldTicks()).toBeGreaterThan(0), { timeout: WAIT_TIMEOUT_MS });
  },
  WAIT_TIMEOUT_MS * 2 + 10_000,
);
