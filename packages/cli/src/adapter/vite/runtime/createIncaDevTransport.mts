// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { ModuleRunnerTransport } from "vite/module-runner";

import "./incaDev.mts";

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
