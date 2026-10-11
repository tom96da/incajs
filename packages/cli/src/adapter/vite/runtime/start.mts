// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Provides the globals vite/module-runner needs (TextDecoder/URL); importing it installs them.
import "./globals.mts";
import { ModuleRunner } from "vite/module-runner";

import { createIncaDevTransport } from "./createIncaDevTransport.mts";
import { createIncaEvaluator } from "./createIncaEvaluator.mts";

/** Whether `error` came from the development server, which has already reported it. */
function isServerFailure(error: unknown): boolean {
  return typeof error === "object" && error !== null && "runnerError" in error;
}

/**
 * Builds a `ModuleRunner` wired to the app's `__inca_dev__` channel and
 * imports `entryId` through it.
 *
 * A failed import is retried after the next `file-changed` payload, until
 * one succeeds. A failure the development server reported stays silent. Any
 * other failure is rethrown as an unhandled rejection.
 * `__inca_dev__.setLoadFailed(true)` follows a failed attempt and
 * `setLoadFailed(false)` precedes each retry.
 *
 * Every inlined module runs through `__inca_dev__.compile`; the import fails
 * when the host lacks it.
 *
 * `sourcemapInterceptor: false` is required: the interceptor reads V8
 * `CallSite` objects off `Error.stack`, and this engine's `Error.stack` is a
 * plain string. The host remaps the frames it reports through each module's
 * inline source map.
 *
 * `hmr.logger` is fully muted: the development server already reports each
 * update, and a failed one, on its own — see `hmr.mts`'s `onError`.
 * @param entryId - the module id the development server serves the entry under
 * @returns the entry's exports, once an import succeeds
 */
export async function start(entryId: string): Promise<unknown> {
  let edited = (): void => {};
  const runner = new ModuleRunner(
    {
      transport: { ...createIncaDevTransport(() => edited()), timeout: 0 },
      hmr: { logger: { debug: () => {}, error: () => {} } },
      sourcemapInterceptor: false,
    },
    createIncaEvaluator(),
  );
  for (;;) {
    // Armed before the attempt, so an edit that lands during it is caught.
    const changed = new Promise<void>((resolve) => {
      edited = resolve;
    });
    try {
      return await runner.import(entryId);
    } catch (error) {
      if (!isServerFailure(error)) void Promise.reject(error);
      globalThis.__inca_dev__?.setLoadFailed?.(true);
    }
    await changed;
    // Clears every cached module, so loaded dependencies evaluate again.
    runner.clearCache();
    globalThis.__inca_dev__?.setLoadFailed?.(false);
  }
}
