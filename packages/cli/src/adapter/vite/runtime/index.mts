// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

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
 * Installs `TextDecoder` and `URL` on `globalThis` if the engine doesn't
 * already provide one. `vite/module-runner` constructs a `TextDecoder` at
 * module load and a `URL` per module evaluated; neither is present in
 * QuickJS. Leaves an existing implementation untouched.
 */
export function installRuntimeGlobals(): void {
  if (typeof globalThis.TextDecoder === "undefined") {
    globalThis.TextDecoder = class TextDecoder {
      /**
       * Not a real UTF-8 decode — only `module-runner`'s inline-sourcemap
       * decoding calls it, and this runtime never reaches that path.
       */
      decode(bytes: Iterable<number>): string {
        let out = "";
        for (const byte of bytes) out += String.fromCharCode(byte);
        return out;
      }
    } as any;
  }

  if (typeof globalThis.URL === "undefined") {
    globalThis.URL = class URL {
      /**
       * Not spec-compliant: only `.href`, which is all `module-runner`
       * reads back; a `base` argument is ignored.
       */
      href: string;
      constructor(input: string) {
        this.href = input;
      }
    } as any;
  }
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
 * Installs the runtime globals, builds a `ModuleRunner` wired to the app's
 * `__inca_dev__` channel, and imports `entryId` through it.
 *
 * `sourcemapInterceptor: false` is required, not just faster: the
 * alternative reads V8 `CallSite` objects off `Error.stack`, and this
 * engine's `Error.stack` is a plain string, not `CallSite` objects.
 */
export function start(entryId: string): Promise<unknown> {
  installRuntimeGlobals();
  const runner = new ModuleRunner(
    {
      transport: { ...createIncaDevTransport(), timeout: 0 },
      hmr: true,
      sourcemapInterceptor: false,
    },
    new ESModulesEvaluator(),
  );
  return runner.import(entryId);
}
