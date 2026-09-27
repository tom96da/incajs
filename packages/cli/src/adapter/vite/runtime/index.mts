// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Provides the globals vite/module-runner needs (TextDecoder/URL).
import "./globals.mts";
import { ESModulesEvaluator, ModuleRunner } from "vite/module-runner";
import type { ModuleRunnerTransport } from "vite/module-runner";

/**
 * A running app's own `globalThis.__inca_dev__`, installed by the host in
 * dev builds only.
 */
interface IncaDev {
  send: (method: string, paramsJson: string) => void;
  receive?: (method: string, paramsJson: string) => void;
}

declare global {
  // eslint-disable-next-line no-var
  var __inca_dev__: IncaDev | undefined;
}

/**
 * A `ModuleRunnerTransport` that carries Vite's runner protocol over
 * `globalThis.__inca_dev__`, nested under the `"vite"` method name so the
 * host's own dev-protocol channel stays bundler-agnostic.
 */
export function createIncaDevTransport(): Required<
  Pick<ModuleRunnerTransport, "connect" | "send">
> {
  return {
    connect({ onMessage }) {
      const dev = globalThis.__inca_dev__;
      if (!dev) throw new Error("__inca_dev__ is not installed — this build is not a dev build");
      // No disconnect signal exists on this channel today, so there's
      // nothing to call `onDisconnection` for.
      dev.receive = (method, paramsJson) => {
        if (method !== "vite") return;
        onMessage(JSON.parse(paramsJson));
      };
    },
    send(data) {
      const dev = globalThis.__inca_dev__;
      if (!dev) throw new Error("__inca_dev__ is not installed — this build is not a dev build");
      dev.send("vite", JSON.stringify(data));
    },
  };
}

/**
 * Builds a `ModuleRunner` wired to the app's `__inca_dev__` channel and
 * imports `entryId` through it. `./globals.mts`'s import above already
 * installed the runtime globals `vite/module-runner` needs.
 *
 * `sourcemapInterceptor: false` is required, not just faster: the
 * alternative reads V8 `CallSite` objects off `Error.stack`, and this
 * engine's `Error.stack` is a plain string, not `CallSite` objects.
 *
 * `hmr.logger.debug` is muted: the dev server already reports each update
 * on its own line. `error` stays wired to `console.error` — a failed
 * update still needs to be visible.
 */
export function start(entryId: string): Promise<unknown> {
  const runner = new ModuleRunner(
    {
      transport: { ...createIncaDevTransport(), timeout: 0 },
      hmr: { logger: { debug: () => {}, error: (msg) => console.error(msg) } },
      sourcemapInterceptor: false,
    },
    new ESModulesEvaluator(),
  );
  return runner.import(entryId);
}
