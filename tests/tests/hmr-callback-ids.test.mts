// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Drives the real host binary through a failed first load that the
// dev server retries in the same engine. Listeners registered before and
// after the retry keep their own handlers. Console output is the signal.

import { existsSync, readFileSync } from "node:fs";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";

import { afterAll, beforeAll, expect, it, onTestFailed, vi } from "vitest";

import { defaultBundler } from "../../packages/cli/src/defaultBundler.mts";
import { HostClient } from "../../packages/cli/src/dev-client/hostClient.mts";
import { resolveTestHostBin } from "../support/hostBin.mts";
import { reloadHost, settleReloads } from "../support/reloads.mts";
import type { BuildFailure, HmrChannel } from "../../packages/cli/src/adapter/types.mts";

const RELOAD_TIMEOUT_MS = 10_000;
/** Generous: a real process spawn, a real dev server and a real file watcher sit in the loop. */
const WAIT_TIMEOUT_MS = 30_000;

function discard(): Writable {
  return new Writable({
    write(_chunk, _encoding, callback) {
      callback();
    },
  });
}

/** The fixture imports the built `dist/` of both packages. Fails with a message if missing. */
function checkJsPackagesAreBuilt(): void {
  const repoRoot = path.resolve(import.meta.dirname, "../..");
  const missing = [
    path.join(repoRoot, "packages/core/dist"),
    path.join(repoRoot, "packages/cli/dist/hmr-runtime.js"),
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
import "./early.mts";
import "./late.mts";
import App from "./App.vue";

createIncaApp(App).mount();
`;

const APP_VUE = readFileSync(path.join(import.meta.dirname, "fixtures/callback-ids.vue"), "utf8");

const EARLY_MTS = `
import { window } from "incajs";

// Only the first evaluation registers. The retry evaluates this module again.
// Editing this module sends a full reload, which starts a fresh engine and drops the registration.
if (!globalThis.__e2eEarlySeen) {
  globalThis.__e2eEarlySeen = true;
  window.addEventListener("ping", () => console.log("[e2e] early ran"));
  console.log("[e2e] early registered");
}
`;

const LATE_MTS = `
import "./broken.mts";
import { window } from "incajs";

window.addEventListener("pong", () => console.log("[e2e] late ran"));
`;

const scratchRoot = path.join(import.meta.dirname, "tmp", "hmr-callback-ids");
let fixtureDir: string;
let channel: HmrChannel | undefined;
let hostClient: HostClient | undefined;
let stderrText = "";
const buildErrors: BuildFailure[] = [];

beforeAll(async () => {
  await mkdir(scratchRoot, { recursive: true });
  fixtureDir = await mkdtemp(path.join(scratchRoot, "app-"));
  const entry = path.join(fixtureDir, "entry.mts");
  await writeFile(path.join(fixtureDir, "App.vue"), APP_VUE);
  await writeFile(path.join(fixtureDir, "early.mts"), EARLY_MTS);
  await writeFile(path.join(fixtureDir, "late.mts"), LATE_MTS);
  await writeFile(path.join(fixtureDir, "broken.mts"), "export const = ;\n");
  await writeFile(entry, ENTRY_MTS);

  channel = await defaultBundler.hmr!({
    entry,
    cwd: fixtureDir,
    stdout: discard(),
    stderr: discard(),
    quiet: true,
    notify: (payload) => hostClient?.notify("vite", payload),
    reload: () => reloadHost(hostClient, RELOAD_TIMEOUT_MS),
    onError: (error) => buildErrors.push(error),
  });

  hostClient = new HostClient({
    entryFile: channel.entryFile,
    hostBin,
    onStderr: (line) => {
      stderrText += line;
    },
    integrations: { vite: (params) => channel?.dispatch(params) },
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
  "keeps each listener on its own handler after a failed first load is retried",
  async () => {
    const client = hostClient!;
    onTestFailed(() => console.error(`--- host stderr ---\n${stderrText}`));

    await vi.waitFor(() => expect(buildErrors).toHaveLength(1), { timeout: WAIT_TIMEOUT_MS });
    await vi.waitFor(() => expect(stderrText).toContain("[e2e] early registered"), {
      timeout: WAIT_TIMEOUT_MS,
    });

    await writeFile(path.join(fixtureDir, "broken.mts"), "export {};\n");
    await vi.waitFor(() => expect(stderrText).toContain("[e2e] mounted"), {
      timeout: WAIT_TIMEOUT_MS,
    });

    client.notify("tick", {});
    await vi.waitFor(() => expect(stderrText).toContain("[e2e] dispatched ping"), {
      timeout: WAIT_TIMEOUT_MS,
    });
    expect(stderrText).not.toContain("[e2e] late ran");
    await vi.waitFor(() => expect(stderrText).toContain("[e2e] early ran"), {
      timeout: WAIT_TIMEOUT_MS,
    });

    client.notify("tick", {});
    await vi.waitFor(() => expect(stderrText).toContain("[e2e] late ran"), {
      timeout: WAIT_TIMEOUT_MS,
    });
  },
  WAIT_TIMEOUT_MS * 3,
);
