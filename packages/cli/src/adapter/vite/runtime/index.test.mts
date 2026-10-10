// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { build, mergeConfig } from "vite";
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import viteConfig from "../../../../vite.config.mts";
import { start } from "./index.mts";

const BARE_IMPORT_RE = /^\s*import\s+(?:type\s+)?[^"'()]*from\s*["']([^"']+)["']/gm;

function isRelative(specifier: string): boolean {
  return specifier.startsWith("./") || specifier.startsWith("../");
}

/**
 * Every import specifier a built file's `import ... from "..."` statements
 * use. Drops matches inside a template-literal string the built code
 * happens to contain (a specifier can't hold `${`, so those aren't real
 * import declarations).
 */
function importSpecifiers(source: string): string[] {
  return [...source.matchAll(BARE_IMPORT_RE)]
    .map((match) => match[1] ?? "")
    .filter((specifier) => !specifier.includes("${"));
}

/**
 * Reads `entryFile` and every file it imports, transitively following only
 * relative specifiers, and returns the set of files visited.
 */
async function readTransitiveImports(entryFile: string): Promise<Map<string, string>> {
  const visited = new Map<string, string>();
  const queue = [entryFile];
  while (queue.length > 0) {
    const file = queue.pop();
    if (!file || visited.has(file)) continue;
    const source = await readFile(file, "utf8");
    visited.set(file, source);
    for (const specifier of importSpecifiers(source)) {
      if (isRelative(specifier)) queue.push(path.resolve(path.dirname(file), specifier));
    }
  }
  return visited;
}

describe("hmr-runtime build output", () => {
  let outDir: string;

  beforeAll(async () => {
    outDir = await mkdtemp(path.join(tmpdir(), "inca-hmr-runtime-"));
    await build(
      mergeConfig(viteConfig, {
        build: { outDir, emptyOutDir: true },
        plugins: [],
        logLevel: "silent",
      }),
    );
  }, 30000);

  afterAll(async () => {
    await rm(outDir, { recursive: true, force: true });
  });

  it("bundles vite/module-runner in, leaving no bare or node: import behind", async () => {
    const files = await readTransitiveImports(path.join(outDir, "hmr-runtime.js"));

    expect(files.size).toBeGreaterThan(0);
    for (const [file, source] of files) {
      for (const specifier of importSpecifiers(source)) {
        expect(isRelative(specifier), `${file} imports "${specifier}"`).toBe(true);
      }
    }
  });
});

type Fetch = () => Promise<unknown> | unknown;

/** What the development server answers a `fetchModule` with when the module compiles. */
function module(code: string): unknown {
  return { code, file: "/entry.js", id: "/entry.js", url: "/entry.js", invalidate: false };
}

/**
 * Installs a fake `__inca_dev__` that answers each `fetchModule` for the
 * entry with the next of `fetches` (a thrown value becomes an error
 * response) and records every `setLoadFailed` call in `calls`.
 */
function installDev(fetches: Fetch[]) {
  const calls: boolean[] = [];
  let fetched = 0;
  const dev: NonNullable<typeof globalThis.__inca_dev__> = {
    setLoadFailed: (failed) => calls.push(failed),
    send(_method, paramsJson) {
      const { data } = JSON.parse(paramsJson) as {
        data: { id: string; name: string };
      };
      const id = `response:${data.id.slice("send:".length)}`;
      const reply = (body: object): void =>
        dev.receive?.(
          "vite",
          JSON.stringify({ type: "custom", event: "vite:invoke", data: { id, data: body } }),
        );
      // The runner registers its pending call after `send` returns, so a reply is never synchronous.
      void Promise.resolve()
        .then(() => (data.name === "getBuiltins" ? [] : fetches[fetched++]?.()))
        .then(
          (result) => reply({ result }),
          (error: Error) => reply({ error: { message: error.message } }),
        );
    },
  };
  globalThis.__inca_dev__ = dev;
  return {
    calls,
    fetched: () => fetched,
    edit: () =>
      dev.receive?.(
        "vite",
        JSON.stringify({ type: "custom", event: "file-changed", data: { file: "/a.vue" } }),
      ),
  };
}

describe("start", () => {
  const rejected: unknown[] = [];

  function captureRejections(): void {
    const reject = Promise.reject.bind(Promise);
    vi.spyOn(Promise, "reject").mockImplementation((error) => {
      rejected.push(error);
      const promise = reject(error);
      promise.catch(() => {});
      return promise;
    });
  }

  afterEach(() => {
    rejected.length = 0;
    vi.restoreAllMocks();
    globalThis.__inca_dev__ = undefined;
  });

  it("retries after an edit when the development server failed, staying silent", async () => {
    captureRejections();
    const dev = installDev([
      () => {
        throw new Error("compile error");
      },
      () => module("return 1"),
    ]);

    const started = start("/entry.js");
    await vi.waitFor(() => expect(dev.calls).toEqual([true]));
    expect(dev.fetched()).toBe(1);

    dev.edit();
    await started;

    expect(dev.fetched()).toBe(2);
    expect(dev.calls).toEqual([true, false]);
    expect(rejected).toEqual([]);
  });

  it("retries an attempt that an edit landed during", async () => {
    let release = (): void => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const dev = installDev([
      async () => {
        await gate;
        throw new Error("compile error");
      },
      () => module("return 1"),
    ]);

    const started = start("/entry.js");
    await vi.waitFor(() => expect(dev.fetched()).toBe(1));
    dev.edit();
    release();
    await started;

    expect(dev.fetched()).toBe(2);
    expect(dev.calls).toEqual([true, false]);
  });

  it("reports an error the app threw once, then recovers after an edit", async () => {
    captureRejections();
    const dev = installDev([() => module('throw new Error("boom")'), () => module("return 1")]);

    const started = start("/entry.js");
    await vi.waitFor(() => expect(dev.calls).toEqual([true]));
    expect(rejected).toHaveLength(1);
    expect(rejected[0]).toMatchObject({ message: "boom" });

    dev.edit();
    await started;

    expect(dev.calls).toEqual([true, false]);
    expect(rejected).toHaveLength(1);
  });

  it("retries again when the retry fails too", async () => {
    captureRejections();
    const fail = (): never => {
      throw new Error("compile error");
    };
    const dev = installDev([fail, fail, () => module("return 1")]);

    const started = start("/entry.js");
    await vi.waitFor(() => expect(dev.calls).toEqual([true]));
    dev.edit();
    await vi.waitFor(() => expect(dev.calls).toEqual([true, false, true]));
    dev.edit();
    await started;

    expect(dev.calls).toEqual([true, false, true, false]);
    expect(dev.fetched()).toBe(3);
    expect(rejected).toEqual([]);
  });

  it("reports a thrown non-Error value once, then recovers after an edit", async () => {
    captureRejections();
    const dev = installDev([() => module('throw "x"'), () => module("return 1")]);

    const started = start("/entry.js");
    await vi.waitFor(() => expect(dev.calls).toEqual([true]));
    expect(rejected).toEqual(["x"]);

    dev.edit();
    await started;

    expect(dev.calls).toEqual([true, false]);
  });

  it("makes no setLoadFailed call when the first import succeeds", async () => {
    const dev = installDev([() => module("return 1")]);

    await start("/entry.js");

    expect(dev.calls).toEqual([]);
  });

  it("recovers when the host lacks setLoadFailed", async () => {
    captureRejections();
    const dev = installDev([
      () => {
        throw new Error("compile error");
      },
      () => module("__vite_ssr_exports__.answer = 42;"),
    ]);
    delete globalThis.__inca_dev__!.setLoadFailed;

    const started = start("/entry.js");
    await vi.waitFor(() => expect(dev.fetched()).toBe(1));
    dev.edit();

    await expect(started).resolves.toMatchObject({ answer: 42 });
    expect(rejected).toEqual([]);
  });
});
