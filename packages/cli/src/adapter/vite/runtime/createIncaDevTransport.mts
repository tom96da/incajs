// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { ModuleRunnerTransport } from "vite/module-runner";

const NO_HOST =
  "this code needs a host that serves the development protocol. start the app with `inca dev`.";

/**
 * A `ModuleRunnerTransport` that carries Vite's runner protocol over
 * `globalThis.__inca_dev__`, nested under the `"vite"` method name so the
 * host's own development protocol channel stays bundler-agnostic.
 * @param onFileChanged - runs on each `file-changed` payload before the
 * payload is forwarded to the runner
 * @returns the `connect` and `send` methods of a `ModuleRunnerTransport`
 */
export function createIncaDevTransport(
  onFileChanged?: () => void,
): Required<Pick<ModuleRunnerTransport, "connect" | "send">> {
  return {
    connect({ onMessage }) {
      const dev = globalThis.__inca_dev__;
      if (!dev) throw new Error(NO_HOST);
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
      if (!dev) throw new Error(NO_HOST);
      dev.send("vite", JSON.stringify(data));
    },
  };
}
