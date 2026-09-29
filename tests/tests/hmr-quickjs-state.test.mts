// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Drives the real `inca-host` binary and a real Vite dev server through an
// HMR update — the one piece of experimental HMR that genuinely spans Rust
// and TypeScript.
//
// Checks state through QuickJS console output only. Whether an update
// actually reaches the screen is outside this test's reach.

import { existsSync, readFileSync } from "node:fs";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";

import { afterAll, beforeAll, expect, it, onTestFailed, vi } from "vitest";

import { hmr } from "../../packages/cli/src/adapter/vite/index.mts";
import { HostClient } from "../../packages/cli/src/dev-client/hostClient.mts";
import type { HmrChannel, UpdateError } from "../../packages/cli/src/adapter/types.mts";
import type { AppErrorParams } from "../../packages/cli/src/dev-client/protocol.mts";

/** How long the host is given to reload/relay before this test gives up on it. */
const RELOAD_TIMEOUT_MS = 10_000;
/** Generous: a real process spawn, a real dev server, and a real file watcher all sit in the loop. */
const WAIT_TIMEOUT_MS = 30_000;

/** How many times `setup()` has logged its mount marker so far. */
function mountedCount(text: string): number {
  return text.match(/\[e2e\] mounted/g)?.length ?? 0;
}

/** A writable that discards everything — silences the dev server's own logging. */
function discard(): Writable {
  return new Writable({
    write(_chunk, _encoding, callback) {
      callback();
    },
  });
}

/**
 * `target/debug/inca-host` (or `target/release/inca-host`, whichever
 * exists), resolved relative to the repo root two directories up from this
 * file. Never built here — fails with a message instead.
 */
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

/**
 * `incajs`'s and `@incajs/cli`'s built `dist/` — the fixture's entry
 * imports both. Never built here — fails with a message instead.
 */
function checkJsPackagesAreBuilt(): void {
  const repoRoot = path.resolve(import.meta.dirname, "../..");
  const missing = [
    path.join(repoRoot, "packages/core/dist"),
    path.join(repoRoot, "packages/cli/dist/hmr-runtime.js"),
  ].filter((p) => !existsSync(p));
  if (missing.length > 0) {
    throw new Error(
      `missing build output: ${missing.join(", ")} — run ` +
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

/**
 * A click-counter-shaped app (ported from examples/click_counter/src/App.vue),
 * driven by a `tick` dev notification instead of a real pointer click — there
 * is no way to synthesize one against a running `inca-host` process from
 * outside. `receive` is chained rather than replaced: `vite/module-runner`'s
 * own transport already claimed `__inca_dev__.receive` for its own traffic
 * before this script runs.
 */
const APP_VUE_TEMPLATE = readFileSync(
  path.join(import.meta.dirname, "fixtures/click-counter.vue"),
  "utf8",
);

function appVue(style: string, labelSuffix: string): string {
  return APP_VUE_TEMPLATE.replace("__LABEL_SUFFIX__", labelSuffix).replace("__STYLE__", style);
}

const STYLE_V1 = `
  display: 'flex',
  justify_content: 'center',
  align_items: 'center',
  width: 300,
  height: 150,
  background: 0x505050,
  border_width: 1,
  border_color: 0x0000ff,
  corner_radius: 8,
  text_color: 0xffffff,
  text_size: 20
`;

// A template-only edit: same script, a different corner_radius/border_color.
const STYLE_V2 = STYLE_V1.replace("border_color: 0x0000ff", "border_color: 0x00ff00").replace(
  "corner_radius: 8",
  "corner_radius: 12",
);

const scratchRoot = path.join(import.meta.dirname, "tmp", "hmr-quickjs-state");
let fixtureDir: string;
let vuePath: string;
let channel: HmrChannel | undefined;
let hostClient: HostClient | undefined;
let stderrText = "";
let notifiedCount = 0;
let ready = false;
let buildErrors: UpdateError[] = [];
let appErrors: AppErrorParams[] = [];

beforeAll(async () => {
  await mkdir(scratchRoot, { recursive: true });
  fixtureDir = await mkdtemp(path.join(scratchRoot, "app-"));
  vuePath = path.join(fixtureDir, "App.vue");
  const entry = path.join(fixtureDir, "entry.mts");
  await writeFile(vuePath, appVue(STYLE_V1, ""));
  await writeFile(entry, ENTRY_MTS);

  channel = await hmr({
    entry,
    cwd: fixtureDir,
    stdout: discard(),
    stderr: discard(),
    quiet: true,
    notify: (payload) => {
      notifiedCount += 1;
      hostClient?.notify("vite", payload);
    },
    // A full reload needs a fresh Session, which only inca-host's own
    // `reload` RPC gives it — not expected to fire for a Vue SFC edit
    // that always has an HMR boundary, but wired for real just in case.
    reload: () => void hostClient?.call("reload", undefined, RELOAD_TIMEOUT_MS),
    onError: (error) => buildErrors.push(error),
  });

  hostClient = new HostClient({
    entryFile: channel.entryFile,
    hostBin,
    onStderr: (line) => {
      stderrText += line;
    },
    onReady: () => {
      ready = true;
    },
    onAppError: (error) => appErrors.push(error),
    integrations: { vite: (params) => channel?.dispatch(params) },
  });
  await hostClient.start();
}, WAIT_TIMEOUT_MS + 10_000);

afterAll(async () => {
  await hostClient?.stop();
  await channel?.close();
  await rm(scratchRoot, { recursive: true, force: true });
});

it(
  "keeps Vue component state across a template-only HMR edit, resets it across a script edit",
  async () => {
    const client = hostClient!;
    onTestFailed(() => console.error(`--- host stderr ---\n${stderrText}`));

    await vi.waitFor(() => expect(ready).toBe(true), { timeout: WAIT_TIMEOUT_MS });
    await vi.waitFor(() => expect(stderrText).toContain("[e2e] mounted"), {
      timeout: WAIT_TIMEOUT_MS,
    });

    client.notify("tick", {});
    await vi.waitFor(() => expect(stderrText).toContain("[e2e] clicks=1"), {
      timeout: WAIT_TIMEOUT_MS,
    });
    client.notify("tick", {});
    await vi.waitFor(() => expect(stderrText).toContain("[e2e] clicks=2"), {
      timeout: WAIT_TIMEOUT_MS,
    });

    // Template-only edit: no new setup() call, so `clicks` must survive.
    const beforeTemplateEdit = notifiedCount;
    await writeFile(vuePath, appVue(STYLE_V2, ""));
    await vi.waitFor(() => expect(notifiedCount).toBeGreaterThan(beforeTemplateEdit), {
      timeout: WAIT_TIMEOUT_MS,
    });

    client.notify("tick", {});
    await vi.waitFor(() => expect(stderrText).toContain("[e2e] clicks=3"), {
      timeout: WAIT_TIMEOUT_MS,
    });
    client.notify("tick", {});
    await vi.waitFor(() => expect(stderrText).toContain("[e2e] clicks=4"), {
      timeout: WAIT_TIMEOUT_MS,
    });

    // Script edit: a fresh setup() call resets `clicks` back to 0. Wait on
    // the mount marker, not `notify` — `notify` also fires on the earlier
    // `file-changed` event, before the module actually re-evaluates.
    const beforeScriptEdit = mountedCount(stderrText);
    await writeFile(vuePath, appVue(STYLE_V2, "!"));
    await vi.waitFor(() => expect(mountedCount(stderrText)).toBeGreaterThan(beforeScriptEdit), {
      timeout: WAIT_TIMEOUT_MS,
    });

    client.notify("tick", {});
    // Not a leftover from the pre-edit run of clicks=1 — that assertion above
    // already consumed it; a second, later occurrence proves the reset. A
    // trailing-boundary match so a future clicks=10+ can't inflate the count.
    await vi.waitFor(() => expect(stderrText.match(/\[e2e\] clicks=1(?!\d)/g)?.length).toBe(2), {
      timeout: WAIT_TIMEOUT_MS,
    });

    // A broken script edit is reported through `onError` but doesn't take
    // the host down.
    const brokenVue = appVue(STYLE_V2, "!").replace(
      "const clicks = ref(0);",
      "const clicks = ref(0);\nconst oops = (;",
    );
    await writeFile(vuePath, brokenVue);
    await vi.waitFor(() => expect(buildErrors).toHaveLength(1), { timeout: WAIT_TIMEOUT_MS });
    expect(buildErrors[0]?.message).toContain("Unexpected token");

    // Fixing it remounts cleanly and the host keeps responding — same race
    // as the script edit above.
    const beforeRecoveryEdit = mountedCount(stderrText);
    await writeFile(vuePath, appVue(STYLE_V2, "!"));
    await vi.waitFor(() => expect(mountedCount(stderrText)).toBeGreaterThan(beforeRecoveryEdit), {
      timeout: WAIT_TIMEOUT_MS,
    });

    client.notify("tick", {});
    await vi.waitFor(
      () => expect(stderrText.match(/\[e2e\] clicks=1(?!\d)/g)?.length).toBeGreaterThanOrEqual(3),
      { timeout: WAIT_TIMEOUT_MS },
    );
  },
  WAIT_TIMEOUT_MS + 30_000,
);

it(
  "mounts a template of static siblings under HMR",
  async () => {
    const client = hostClient!;
    onTestFailed(() => console.error(`--- host stderr ---\n${stderrText}`));

    await vi.waitFor(() => expect(ready).toBe(true), { timeout: WAIT_TIMEOUT_MS });
    await vi.waitFor(() => expect(stderrText).toContain("[e2e] mounted"), {
      timeout: WAIT_TIMEOUT_MS,
    });

    // The suffix changes the script, which forces a remount.
    const beforeMount = mountedCount(stderrText);
    const statics = `{{ console.log("[e2e] rendered") }}${"<div>a</div>".repeat(25)}`;
    await writeFile(vuePath, appVue(STYLE_V2, "?").replace("{{ label }}", `{{ label }}${statics}`));
    await vi.waitFor(() => expect(mountedCount(stderrText)).toBeGreaterThan(beforeMount), {
      timeout: WAIT_TIMEOUT_MS,
    });
    await vi.waitFor(() => expect(stderrText).toContain("[e2e] rendered"), {
      timeout: WAIT_TIMEOUT_MS,
    });

    // stderr is one ordered stream, so a later line means the mount has finished.
    client.notify("tick", {});
    await vi.waitFor(
      () => expect(stderrText.slice(stderrText.indexOf("[e2e] rendered"))).toContain("clicks="),
      { timeout: WAIT_TIMEOUT_MS },
    );
    expect(appErrors).toEqual([]);
  },
  WAIT_TIMEOUT_MS + 10_000,
);
