// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { chmod, writeFile } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";

import { onTestFinished, vi } from "vitest";

import type {
  Bundler,
  BundlerOptions,
  BuildOutput,
  HmrOptions,
  Watcher,
} from "../../src/adapter/types.mts";

const tests = path.join(import.meta.dirname, "..");

/** A spawned mock host reports late while the whole suite runs in parallel. */
export const WAIT_TIMEOUT_MS = 10_000;
vi.setConfig({ testTimeout: 2 * WAIT_TIMEOUT_MS });

export const waitFor = <T,>(callback: () => T | Promise<T>): Promise<T> =>
  vi.waitFor(callback, WAIT_TIMEOUT_MS);

export const mockHost = path.join(tests, "fixtures/mock-host.mts");
export const envMockHost = path.join(tests, "fixtures/mock-host-env.mts");
export const reloadFailsMockHost = path.join(tests, "fixtures/mock-host-reload-fails.mts");
export const appErrorMockHost = path.join(tests, "fixtures/mock-host-app-error.mts");
export const protocolMismatchMockHost = path.join(
  tests,
  "dev-client/fixtures/mock-host-protocol-mismatch.mts",
);
export const startupFailureMockHost = path.join(
  tests,
  "dev-client/fixtures/mock-host-startup-failure.mts",
);
export const slowReadyMockHost = path.join(tests, "fixtures/mock-host-slow-ready.mts");
export const exitingMockHost = path.join(tests, "fixtures/mock-host-exits.mts");
export const hmrMockHost = path.join(tests, "fixtures/mock-host-hmr.mts");

// A nonexistent directory: pruneStaleFiles() treats a missing outDir as
// nothing to prune, so these tests need no real files on disk.
export const FAKE_OUTPUT: BuildOutput = {
  outDir: "/unused",
  entryFile: "/unused/bundle.js",
  files: ["bundle.js"],
};

export interface FakeBundler extends Bundler {
  /** Simulates a successful (re)build. */
  emitBuild(output?: BuildOutput): void;
  /** Simulates a build failure. */
  emitError(error: { message: string; stack: string | null }): void;
  closed: boolean;
  /**
   * Resolves once `watch()` is called. `dev()` resolves its config
   * (an async step) before reaching `watch()`, so a test firing an event
   * right after calling `dev()` has to wait for this first, or the event
   * lands before any handler is registered to see it.
   */
  watching: Promise<void>;
}

export function makeFakeBundler(): FakeBundler {
  let handlers: Pick<BundlerOptions, "onBuild" | "onError"> | undefined;
  let markWatching: () => void;
  const watching = new Promise<void>((resolve) => {
    markWatching = resolve;
  });

  const fake: FakeBundler = {
    closed: false,
    watching,
    watch(options): Promise<Watcher> {
      handlers = options;
      markWatching();
      return Promise.resolve({
        close: () => {
          fake.closed = true;
          return Promise.resolve();
        },
      });
    },
    build: () => Promise.reject(new Error("not used by dev()")),
    emitBuild(output = FAKE_OUTPUT) {
      handlers?.onBuild(output);
    },
    emitError(error) {
      handlers?.onError(error);
    },
  };
  return fake;
}

/** Reports this process as `name` for one test. */
export function pretendPlatform(name: NodeJS.Platform): void {
  const real = process.platform;
  Object.defineProperty(process, "platform", { value: name, configurable: true });
  onTestFinished(() => {
    Object.defineProperty(process, "platform", { value: real, configurable: true });
  });
}

/** A host binary in `dir` that exists but can't be spawned. */
export async function makeUnspawnableHost(dir: string): Promise<string> {
  const hostBin = path.join(dir, "inca-host");
  await writeFile(hostBin, "#!/bin/sh\n");
  await chmod(hostBin, 0o644);
  return hostBin;
}

export function makeSink(tty = false): { stream: NodeJS.WritableStream; text: () => string } {
  let data = "";
  const stream = new Writable({
    write(chunk: Buffer, _encoding, callback) {
      data += chunk.toString();
      callback();
    },
  });
  if (tty) Object.assign(stream, { isTTY: true });
  return { stream, text: () => data };
}

export interface FakeHmrBundler extends Bundler {
  /** Simulates the running app pushing a `"vite"` payload out to the dev server. */
  emitVite(payload: unknown): void;
  /** Simulates the environment's `HotChannel` seeing a `full-reload` payload. */
  emitFullReload(): void;
  /** Simulates a failed update the bundler reports. */
  emitError(error: { message: string; stack: string | null }): void;
  /** Simulates a successful update the bundler reports. */
  emitUpdate(info: { file: string; took: number }): void;
  /** Every payload `hmr()`'s channel was asked to `dispatch` back into the app. */
  dispatched: unknown[];
}

export function makeFakeHmrBundler(entryFile: string): FakeHmrBundler {
  let options: HmrOptions | undefined;
  const dispatched: unknown[] = [];

  const fake: FakeHmrBundler = {
    watch: () => Promise.reject(new Error("not used by HMR mode")),
    build: () => Promise.reject(new Error("not used by HMR mode")),
    hmr(opts) {
      options = opts;
      return Promise.resolve({
        entryFile,
        dispatch: (payload) => dispatched.push(payload),
        close: () => Promise.resolve(),
      });
    },
    emitVite(payload) {
      options?.notify(payload);
    },
    emitFullReload() {
      options?.reload();
    },
    emitError(error) {
      options?.onError(error);
    },
    emitUpdate(info) {
      options?.onUpdate?.(info);
    },
    dispatched,
  };
  return fake;
}
