// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Drives the real `inca-host` binary and a real Vite dev server through an edit of a `<style>`
// block and a stylesheet.
//
// The app logs the `style` elements under the root container and their text from a timer, and the
// test waits on those lines.

import { access, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";

import dedent from "ts-dedent";
import { afterAll, expect, it, onTestFailed, vi } from "vitest";

import { defaultBundler } from "../../packages/cli/src/defaultPlugins.mts";
import { HostClient } from "../../packages/cli/src/dev-client/client.mts";
import { resolveTestHostBin } from "../support/hostBin.mts";
import { reloadHost, settleReloads } from "../support/reloads.mts";
import type { BuildFailure, HmrChannel } from "../../packages/cli/src/types.mts";

const RELOAD_TIMEOUT_MS = 10_000;
/** Generous: a real process spawn, a real dev server, and a real file watcher sit in the loop. */
const WAIT_TIMEOUT_MS = 30_000;

const repoRoot = path.resolve(import.meta.dirname, "../..");
const missing: string[] = [];
for (const p of [
  path.join(repoRoot, "packages/core/dist"),
  path.join(repoRoot, "packages/cli/dist/vite-runtime.js"),
]) {
  await access(p).catch(() => missing.push(p));
}
if (missing.length > 0) {
  throw new Error(
    `missing build output: ${missing.join(", ")}. run ` +
      "`pnpm --filter incajs --filter @incajs/cli build` first",
  );
}
const hostBin = resolveTestHostBin();

function discard(): Writable {
  return new Writable({
    write(_chunk, _encoding, callback) {
      callback();
    },
  });
}

const ENTRY = dedent`
  import { createIncaApp } from "incajs/vue";
  import App from "./App.vue";

  createIncaApp(App).mount();
`;

/**
 * Logs the count of `style` elements under the root container, their text and the number of `tick`
 * notifications every 100 ms. `__STYLE__` is replaced by the style block.
 */
const APP = dedent`
  <script setup>
  import { onMounted, ref } from "@vue/runtime-core";

  const ticks = ref(0);

  const previousReceive = globalThis.__inca_dev__.receive;
  globalThis.__inca_dev__.receive = (method, paramsJson) => {
    if (method === "tick") ticks.value += 1;
    else previousReceive?.(method, paramsJson);
  };

  onMounted(() => {
    console.log("[e2e] mounted");
    setInterval(() => {
      const native = globalThis.__inca_native__;
      const styles = native.querySelectorAll(native.rootNodeId(), "style");
      const text = styles.map((id) => native.textContent(id).replace(/\\s+/g, " ").trim()).join("|");
      console.log(\`[e2e] styles=\${styles.length} text=\${JSON.stringify(text)} ticks=\${ticks.value}\`);
    }, 100);
  });
  </script>

  <template>
    <div class="box" />
  </template>

  __STYLE__
`;

const SHEET_V1 = ".box { width: 40px }\n.tall { height: 25px }\n";
const SHEET_V2 = ".box { width: 60px }\n";
/** What the app logs for a sheet: Vite's whitespace is collapsed. */
const LOGGED: Record<string, string> = {
  [SHEET_V1]: ".box { width: 40px } .tall { height: 25px }",
  [SHEET_V2]: ".box { width: 60px }",
};

const line = (css: string, ticks: number): string =>
  `[e2e] styles=1 text=${JSON.stringify(LOGGED[css])} ticks=${ticks}`;

function app(css: string): string {
  return APP.replace("__STYLE__", `<style>\n${css}</style>`);
}

const scratchRoot = path.join(import.meta.dirname, "tmp", "hmr-style");

interface Session {
  dir: string;
  channel: HmrChannel;
  client: HostClient;
  stderr: () => string;
  buildErrors: BuildFailure[];
  /** The payloads the dev server sent to the host. */
  sent: unknown[];
}

const sessions: Session[] = [];

/** Starts a host on a fresh fixture holding `files`, with `entry.mts` as the entry. */
async function startSession(files: Record<string, string>): Promise<Session> {
  await mkdir(scratchRoot, { recursive: true });
  const dir = await mkdtemp(path.join(scratchRoot, "app-"));
  for (const [name, text] of Object.entries(files)) await writeFile(path.join(dir, name), text);

  let stderr = "";
  const session = { buildErrors: [], sent: [] } as unknown as Session;
  const channel = await defaultBundler.hmr!({
    entry: path.join(dir, "entry.mts"),
    cwd: dir,
    stdout: discard(),
    stderr: discard(),
    quiet: true,
    notify: (payload) => {
      session.sent.push(payload);
      session.client?.notify("vite", payload);
    },
    reload: () => reloadHost(session.client, RELOAD_TIMEOUT_MS),
    onError: (error) => session.buildErrors.push(error),
  });
  const client = new HostClient({
    entryFile: channel.entryFile,
    hostBin,
    env: process.env,
    onStderr: (line) => {
      stderr += line;
    },
    integrations: { vite: (params) => channel.dispatch(params) },
  });
  Object.assign(session, { dir, channel, client, stderr: () => stderr });
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
    await rm(scratchRoot, { recursive: true, force: true });
  }
});

it(
  "applies an edited style block without remounting",
  async () => {
    const session = await startSession({ "entry.mts": ENTRY, "App.vue": app(SHEET_V1) });
    onTestFailed(() => console.error(`--- host stderr ---\n${session.stderr()}`));

    await vi.waitFor(() => expect(session.stderr()).toContain(line(SHEET_V1, 0)), {
      timeout: WAIT_TIMEOUT_MS,
    });
    session.client.notify("tick", {});
    session.client.notify("tick", {});
    await vi.waitFor(() => expect(session.stderr()).toContain(line(SHEET_V1, 2)), {
      timeout: WAIT_TIMEOUT_MS,
    });

    await writeFile(path.join(session.dir, "App.vue"), app(SHEET_V2));
    await vi.waitFor(() => expect(session.stderr()).toContain(line(SHEET_V2, 2)), {
      timeout: WAIT_TIMEOUT_MS,
    });
    expect(session.stderr().match(/\[e2e\] mounted/g)).toHaveLength(1);
  },
  WAIT_TIMEOUT_MS * 2 + 10_000,
);

const RETRY_ENTRY = dedent`
  import "./a.css";
  import "./b.mts";
  import { createIncaApp } from "incajs/vue";
  import App from "./App.vue";

  createIncaApp(App).mount();
`;

/** Matches the `file-changed` payload of a file. */
const isChangeOf =
  (name: string) =>
  (payload: unknown): boolean => {
    const { event, data } = payload as { event?: string; data?: { file?: string } };
    return event === "file-changed" && Boolean(data?.file?.endsWith(`/${name}`));
  };

it(
  "keeps one style element and the new rules when the first load failed",
  async () => {
    const session = await startSession({
      "entry.mts": RETRY_ENTRY,
      "App.vue": APP.replace("__STYLE__", ""),
      "a.css": SHEET_V1,
      "b.mts": "export const b = ;\n",
    });
    onTestFailed(() => console.error(`--- host stderr ---\n${session.stderr()}`));

    await vi.waitFor(() => expect(session.buildErrors).not.toHaveLength(0), {
      timeout: WAIT_TIMEOUT_MS,
    });
    await writeFile(path.join(session.dir, "a.css"), SHEET_V2);
    await vi.waitFor(() => expect(session.sent.some(isChangeOf("a.css"))).toBe(true), {
      timeout: WAIT_TIMEOUT_MS,
    });
    await writeFile(path.join(session.dir, "b.mts"), "export const b = 1;\n");

    await vi.waitFor(() => expect(session.stderr()).toContain(line(SHEET_V2, 0)), {
      timeout: WAIT_TIMEOUT_MS,
    });
  },
  WAIT_TIMEOUT_MS * 2 + 10_000,
);
