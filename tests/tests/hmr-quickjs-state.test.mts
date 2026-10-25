// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Drives the real `inca-host` binary and a real Vite dev server through an
// HMR update. This is the one piece of experimental HMR that spans Rust and
// TypeScript.
//
// It checks state through QuickJS console output only. Whether an update
// reaches the screen is outside this test's reach.

import { existsSync, readFileSync } from "node:fs";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";

import { afterAll, beforeAll, describe, expect, it, onTestFailed, vi } from "vitest";

import { defaultBundler } from "../../packages/cli/src/defaultBundler.mts";
import { HostClient } from "../../packages/cli/src/dev-client/hostClient.mts";
import { resolveTestHostBin } from "../support/hostBin.mts";
import { reloadHost, settleReloads } from "../support/reloads.mts";
import type { AppErrorParams } from "../../packages/cli/src/dev-client/protocol.mts";
import type { BuildFailure, HmrChannel } from "../../packages/cli/src/types.mts";

/** How long the host is given to reload/relay before this test gives up on it. */
const RELOAD_TIMEOUT_MS = 10_000;
/** Generous: a real process spawn, a real dev server, and a real file watcher all sit in the loop. */
const WAIT_TIMEOUT_MS = 30_000;

/** How many times `setup()` has logged its mount marker so far. */
function mountedCount(text: string): number {
  return text.match(/\[e2e\] mounted/g)?.length ?? 0;
}

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

/**
 * A click-counter-shaped app (ported from examples/click_counter/src/App.vue),
 * driven by a `tick` dev notification, because a real pointer click cannot be
 * synthesized against a running `inca-host` process from outside. `receive`
 * is chained: `vite/module-runner`'s own transport already claimed
 * `__inca_dev__.receive` for its own traffic before this script runs.
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
let buildErrors: BuildFailure[] = [];
let appErrors: AppErrorParams[] = [];

beforeAll(async () => {
  await mkdir(scratchRoot, { recursive: true });
  fixtureDir = await mkdtemp(path.join(scratchRoot, "app-"));
  vuePath = path.join(fixtureDir, "App.vue");
  const entry = path.join(fixtureDir, "entry.mts");
  await writeFile(vuePath, appVue(STYLE_V1, ""));
  await writeFile(entry, ENTRY_MTS);

  channel = await defaultBundler.hmr!({
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
    // `reload` RPC gives it. A Vue SFC edit always has an HMR boundary, so it
    // is not expected to fire here. It is wired for real just in case.
    reload: () => reloadHost(hostClient, RELOAD_TIMEOUT_MS),
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
  "mounts without Vue's feature flag warning on the host stderr",
  async () => {
    onTestFailed(() => console.error(`--- host stderr ---\n${stderrText}`));

    await vi.waitFor(() => expect(stderrText).toContain("[e2e] mounted"), {
      timeout: WAIT_TIMEOUT_MS,
    });
    expect(stderrText).not.toContain("Feature flag");
  },
  WAIT_TIMEOUT_MS + 10_000,
);

it(
  "keeps component state across a template edit, resets it across a script edit",
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
    // the mount marker. `notify` also fires on the earlier `file-changed`
    // event, before the module re-evaluates.
    const beforeScriptEdit = mountedCount(stderrText);
    await writeFile(vuePath, appVue(STYLE_V2, "!"));
    await vi.waitFor(() => expect(mountedCount(stderrText)).toBeGreaterThan(beforeScriptEdit), {
      timeout: WAIT_TIMEOUT_MS,
    });

    client.notify("tick", {});
    // The assertion above already consumed the pre-edit run of clicks=1, so a
    // second, later occurrence proves the reset. The trailing-boundary match
    // keeps a future clicks=10+ from inflating the count.
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

    // Fixing it remounts cleanly and the host keeps responding. The wait has
    // the same race as the script edit above.
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

describe("a session that starts on a broken source", () => {
  const BROKEN_APP = appVue(STYLE_V1, "").replace(
    "const clicks = ref(0);",
    "const clicks = ref(0);\nconst oops = (;",
  );
  const THROWING_ENTRY = `throw new Error("startup boom");\n`;
  // The throw is on line 5 and `Error` starts at column 11. The compiler strips
  // the interface, so the generated line differs.
  const MAPPED_ENTRY = `interface Shape {\n  size: number;\n}\nconst shape: Shape = { size: 1 };\nthrow new Error("mapped boom");\n`;

  interface Session {
    dir: string;
    channel: HmrChannel;
    client: HostClient;
    stderr: () => string;
    buildErrors: BuildFailure[];
    appErrors: AppErrorParams[];
    ready: () => boolean;
  }

  const sessions: Session[] = [];

  /** Starts a host on a fresh fixture holding `entryText` and `appText`. */
  async function startSession(entryText: string, appText: string): Promise<Session> {
    const dir = await mkdtemp(path.join(scratchRoot, "broken-"));
    const entry = path.join(dir, "entry.mts");
    await writeFile(path.join(dir, "App.vue"), appText);
    await writeFile(entry, entryText);

    let stderr = "";
    let isReady = false;
    const session = { buildErrors: [], appErrors: [] } as unknown as Session;
    const channel = await defaultBundler.hmr!({
      entry,
      cwd: dir,
      stdout: discard(),
      stderr: discard(),
      quiet: true,
      notify: (payload) => session.client?.notify("vite", payload),
      reload: () => reloadHost(session.client, RELOAD_TIMEOUT_MS),
      onError: (error) => session.buildErrors.push(error),
    });
    const client = new HostClient({
      entryFile: channel.entryFile,
      hostBin,
      onStderr: (line) => {
        stderr += line;
      },
      onReady: () => {
        isReady = true;
      },
      onAppError: (error) => session.appErrors.push(error),
      integrations: { vite: (params) => channel.dispatch(params) },
    });
    Object.assign(session, {
      dir,
      channel,
      client,
      stderr: () => stderr,
      ready: () => isReady,
    });
    sessions.push(session);
    await client.start();
    return session;
  }

  afterAll(async () => {
    try {
      for (const { channel } of sessions) await channel.close();
      await settleReloads();
    } finally {
      for (const { client } of sessions) await client.stop();
    }
  });

  it(
    "reports the failure once, and mounts once the file is fixed",
    async () => {
      const session = await startSession(ENTRY_MTS, BROKEN_APP);
      onTestFailed(() => console.error(`--- host stderr ---\n${session.stderr()}`));

      await vi.waitFor(() => expect(session.buildErrors).toHaveLength(1), {
        timeout: WAIT_TIMEOUT_MS,
      });
      expect(session.buildErrors[0]?.message).toContain("Unexpected token");
      expect(session.appErrors).toEqual([]);
      expect(mountedCount(session.stderr())).toBe(0);
      expect(session.ready()).toBe(false);

      await writeFile(path.join(session.dir, "App.vue"), appVue(STYLE_V1, ""));
      await vi.waitFor(() => expect(mountedCount(session.stderr())).toBe(1), {
        timeout: WAIT_TIMEOUT_MS,
      });
      await vi.waitFor(() => expect(session.ready()).toBe(true), { timeout: WAIT_TIMEOUT_MS });

      session.client.notify("tick", {});
      await vi.waitFor(() => expect(session.stderr()).toContain("[e2e] clicks=1"), {
        timeout: WAIT_TIMEOUT_MS,
      });
      expect(session.buildErrors).toHaveLength(1);
      expect(session.appErrors).toEqual([]);
    },
    WAIT_TIMEOUT_MS * 2 + 10_000,
  );

  it(
    "reports an entry that throws once, and mounts once the entry is fixed",
    async () => {
      const session = await startSession(THROWING_ENTRY, appVue(STYLE_V1, ""));
      onTestFailed(() => console.error(`--- host stderr ---\n${session.stderr()}`));

      await vi.waitFor(() => expect(session.appErrors).toHaveLength(1), {
        timeout: WAIT_TIMEOUT_MS,
      });
      expect(session.appErrors[0]?.message).toContain("startup boom");
      // The stack names the module by its file.
      expect(session.appErrors[0]?.stack).toContain("entry.mts:");
      expect(session.appErrors[0]?.stack).not.toContain("<input>");
      expect(session.buildErrors).toEqual([]);
      expect(session.ready()).toBe(false);

      await writeFile(path.join(session.dir, "entry.mts"), ENTRY_MTS);
      await vi.waitFor(() => expect(mountedCount(session.stderr())).toBe(1), {
        timeout: WAIT_TIMEOUT_MS,
      });
      await vi.waitFor(() => expect(session.ready()).toBe(true), { timeout: WAIT_TIMEOUT_MS });

      session.client.notify("tick", {});
      await vi.waitFor(() => expect(session.stderr()).toContain("[e2e] clicks=1"), {
        timeout: WAIT_TIMEOUT_MS,
      });
      expect(session.appErrors).toHaveLength(1);
      expect(session.buildErrors).toEqual([]);
    },
    WAIT_TIMEOUT_MS * 2 + 10_000,
  );

  it(
    "reports an entry's stack at its source line and column",
    async () => {
      const session = await startSession(MAPPED_ENTRY, appVue(STYLE_V1, ""));
      onTestFailed(() => console.error(`--- host stderr ---\n${session.stderr()}`));

      await vi.waitFor(() => expect(session.appErrors).toHaveLength(1), {
        timeout: WAIT_TIMEOUT_MS,
      });
      expect(session.appErrors[0]?.message).toContain("mapped boom");
      expect(session.appErrors[0]?.stack).toMatch(/entry\.mts:5:11/);
    },
    WAIT_TIMEOUT_MS + 10_000,
  );
});
