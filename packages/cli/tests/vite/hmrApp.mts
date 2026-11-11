// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { rm } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";

import { build } from "tsdown";
import { ESModulesEvaluator, ModuleRunner } from "vite/module-runner";
import { afterAll, afterEach, beforeAll } from "vitest";
import type { HotPayload } from "vite";
import type { ModuleRunnerTransport } from "vite/module-runner";

import * as adapterCore from "../../src/adapterCore.mts";
import { createViteBundler } from "../../src/vite/index.mts";
import tsdownConfig from "../../tsdown.config.mts";
import { scratchApp, TEST_INTEGRATION } from "./scratchApp.mts";
import type { HmrChannel, HmrOptions } from "../../src/types.mts";

// The real tsdown.config.mts's own "vite-runtime" entry, reused here rather
// than re-typed so this can never silently drift from the real build.
const runtimeEntry = (tsdownConfig.entry as Record<string, string>)["vite-runtime"]!;

/** A stream that drops what it is given. */
export function discard(): Writable {
  return new Writable({ write: (_chunk, _encoding, callback) => callback() });
}

/**
 * Wires a real `vite/module-runner` `ModuleRunner` to an `HmrChannel`, the
 * same round trip `packages/cli/src/vite/runtime` drives from
 * inside a running app. `notify` plays the server's half, `dispatch` the
 * client's.
 */
export function connectRunner(dispatch: HmrChannel["dispatch"]): {
  runner: ModuleRunner;
  notify: (payload: HotPayload) => void;
} {
  let onMessage: ((data: HotPayload) => void) | undefined;
  const transport: ModuleRunnerTransport = {
    connect(handlers) {
      onMessage = handlers.onMessage;
    },
    send: (data) => dispatch(data),
  };
  const runner = new ModuleRunner({ transport, hmr: { logger: false } }, new ESModulesEvaluator());
  return { runner, notify: (payload) => onMessage?.(payload) };
}

/**
 * Registers the hooks of a test file that drives `hmr()`: a scratch app directory and a build
 * of `vite-runtime.js` of its own (named by `name`, so test files running in parallel don't
 * share them), and the closing of every channel pushed to `channels` after each test.
 * @param name - the scratch directory name of the calling test file
 * @returns `hmr`, which wraps the real one with the scratch build as its `runtimePath`;
 * `makeApp` of the scratch app; and `channels`
 */
export function useHmr(name: string) {
  const { setUp, tearDown, makeApp } = scratchApp(name);

  // Builds vite-runtime.js into its own scratch dir, from the package's real
  // tsdown.config.mts, leaving the package's dist/ untouched.
  const runtimeScratchDir = path.join(import.meta.dirname, `tmp/vite-runtime-build-${name}`);
  const builtRuntimePath = path.join(runtimeScratchDir, "vite-runtime.js");

  /** Wraps the real `hmr()`, defaulting `runtimePath` to this test's own scratch build. */
  function hmr(options: HmrOptions, integration = TEST_INTEGRATION): Promise<HmrChannel> {
    return createViteBundler(adapterCore, ["incajs", "vue"], integration).hmr!({
      runtimePath: builtRuntimePath,
      ...options,
    });
  }

  beforeAll(async () => {
    await setUp();
    await build({
      ...tsdownConfig,
      config: false,
      cwd: path.resolve(import.meta.dirname, "../.."),
      entry: { "vite-runtime": runtimeEntry },
      outDir: runtimeScratchDir,
      // A .js runtime entry needs no declarations, and they would re-typecheck the package.
      dts: false,
      logLevel: "silent",
    });
  }, 120000);
  afterAll(async () => {
    await tearDown();
    await rm(runtimeScratchDir, { recursive: true, force: true });
  });

  const channels: HmrChannel[] = [];
  afterEach(async () => {
    await Promise.all(channels.map((channel) => channel.close()));
    channels.length = 0;
  });

  return { hmr, makeApp, channels, builtRuntimePath };
}
