// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { ModuleRunnerTransport, ModuleRunnerTransportHandlers } from "vite/module-runner";

const NO_HOST =
  "this code needs a host that serves the development protocol. start the app with `inca dev`.";

/**
 * The payload types that change modules or report on them, which a hold keeps from a module that
 * is still loading. A custom event can be what a loading module waits on, so it is never held,
 * and `connected`, `ping` and `bundled-dev-update` change nothing in the runner.
 */
const HELD_TYPES = new Set(["update", "prune", "full-reload", "error"]);

/** A `ModuleRunnerTransport` that can hold the module changes it receives. */
export interface IncaDevTransport extends Required<
  Pick<ModuleRunnerTransport, "connect" | "send">
> {
  /** Holds the updates, prunes, full reloads and errors from now on. */
  hold(): void;
  /**
   * Ends the hold.
   * @param deliver - whether the held payloads go to the runner in their order, or are dropped
   */
  release(deliver: boolean): void;
}

/**
 * A `ModuleRunnerTransport` that carries Vite's runner protocol over
 * `globalThis.__inca_dev__`, nested under the `"vite"` method name so the
 * host's own development protocol channel stays bundler-agnostic.
 * @param onFileChanged - runs on each `file-changed` payload as it arrives, held or not
 * @returns the transport
 */
export function createIncaDevTransport(onFileChanged?: () => void): IncaDevTransport {
  let forward: ModuleRunnerTransportHandlers["onMessage"] = () => {};
  let held: Parameters<typeof forward>[0][] | undefined;
  return {
    connect({ onMessage }) {
      const dev = globalThis.__inca_dev__;
      if (!dev) throw new Error(NO_HOST);
      forward = onMessage;
      // No disconnect signal exists on this channel today, so there's
      // nothing to call `onDisconnection` for.
      dev.receive = (method, paramsJson) => {
        if (method !== "vite") return;
        const payload = JSON.parse(paramsJson);
        if (payload?.type === "custom" && payload.event === "file-changed") onFileChanged?.();
        if (held && HELD_TYPES.has(payload?.type)) held.push(payload);
        else onMessage(payload);
      };
    },
    send(data) {
      const dev = globalThis.__inca_dev__;
      if (!dev) throw new Error(NO_HOST);
      dev.send("vite", JSON.stringify(data));
    },
    hold() {
      held ??= [];
    },
    release(deliver) {
      const payloads = held ?? [];
      held = undefined;
      if (deliver) for (const payload of payloads) forward(payload);
    },
  };
}
