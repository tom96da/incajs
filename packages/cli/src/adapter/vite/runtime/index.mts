// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Provides the globals vite/module-runner needs (TextDecoder/URL).
import "./globals.mts";
import { ESModulesEvaluator, ModuleRunner } from "vite/module-runner";
import type { ModuleRunnerTransport } from "vite/module-runner";

/**
 * A running app's own `globalThis.__inca_dev__`, installed by the host
 * under `--dev`.
 */
interface IncaDev {
  send: (method: string, paramsJson: string) => void;
  receive?: (method: string, paramsJson: string) => void;
  setLoadFailed?: (failed: boolean) => void;
}

declare global {
  // eslint-disable-next-line no-var
  var __inca_dev__: IncaDev | undefined;
}

/**
 * A `ModuleRunnerTransport` that carries Vite's runner protocol over
 * `globalThis.__inca_dev__`, nested under the `"vite"` method name so the
 * host's own dev-protocol channel stays bundler-agnostic.
 * @param onFileChanged - runs on each `file-changed` payload before the
 * payload is forwarded to the runner
 */
export function createIncaDevTransport(
  onFileChanged?: () => void,
): Required<Pick<ModuleRunnerTransport, "connect" | "send">> {
  return {
    connect({ onMessage }) {
      const dev = globalThis.__inca_dev__;
      if (!dev) throw new Error("__inca_dev__ is not installed — this build is not a dev build");
      // No disconnect signal exists on this channel today, so there's
      // nothing to call `onDisconnection` for.
      dev.receive = (method, paramsJson) => {
        if (method !== "vite") return;
        const payload = JSON.parse(paramsJson);
        if (payload?.type === "custom" && payload.event === "file-changed") onFileChanged?.();
        onMessage(payload);
      };
    },
    send(data) {
      const dev = globalThis.__inca_dev__;
      if (!dev) throw new Error("__inca_dev__ is not installed — this build is not a dev build");
      dev.send("vite", JSON.stringify(data));
    },
  };
}

/** Whether `error` came from the development server, which has already reported it. */
function isServerFailure(error: unknown): boolean {
  return typeof error === "object" && error !== null && "runnerError" in error;
}

/**
 * Builds a `ModuleRunner` wired to the app's `__inca_dev__` channel and
 * imports `entryId` through it. `./globals.mts`'s import above already
 * installed the runtime globals `vite/module-runner` needs.
 *
 * A failed import is retried after the next `file-changed` payload, until
 * one succeeds. A failure the development server reported stays silent. Any
 * other failure is rethrown as an unhandled rejection.
 * `__inca_dev__.setLoadFailed(true)` follows a failed attempt and
 * `setLoadFailed(false)` precedes each retry.
 *
 * `sourcemapInterceptor: false` is required: the interceptor reads V8
 * `CallSite` objects off `Error.stack`, and this engine's `Error.stack` is a
 * plain string.
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
    new ESModulesEvaluator(),
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
