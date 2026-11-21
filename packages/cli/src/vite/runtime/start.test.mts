// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, beforeEach, describe, expect, it, onTestFinished, vi } from "vitest";

import { start } from "./start.mts";

const SETTLE_MS = 100;

type Fetch = () => unknown;

/** Stands in for the host's `compile`; it ignores the file name. */
const AsyncFunction = async function () {}.constructor as new (
  ...args: string[]
) => (...args: unknown[]) => Promise<unknown>;

/**
 * What the development server answers a `fetchModule` with when the module compiles.
 * `invalidate` is true when the server's copy changed since the runner last fetched it.
 */
function module(code: string, url = "/entry.js", invalidate = false): unknown {
  return { code, file: url, id: url, url, invalidate };
}

/** The code of a module that logs `name`, accepting its own updates. */
function accepting(name: string): string {
  return `globalThis.__log.push(${JSON.stringify(name)}); __vite_ssr_import_meta__.hot.accept();`;
}

/** The code of a module that imports each of `urls` in order, then logs `"entry"`. */
function importing(...urls: string[]): string {
  return `${urls.map((url) => `await __vite_ssr_import__(${JSON.stringify(url)});`).join(" ")} globalThis.__log.push("entry");`;
}

/** A promise that stays pending until `release` is called. */
function gated(): { gate: Promise<void>; release: () => void } {
  let release = (): void => {};
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  return { gate, release };
}

/** The array the modules of a test log into, as `globalThis.__log`. */
function captureLog(): string[] {
  const log: string[] = [];
  (globalThis as { __log?: string[] }).__log = log;
  onTestFinished(() => {
    delete (globalThis as { __log?: string[] }).__log;
  });
  return log;
}

/**
 * Installs a fake `__inca_dev__` that answers each `fetchModule` with the
 * next of `fetches` (a thrown value becomes an error response), records the
 * fetched urls in `urls` and every `setLoadFailed` call in `calls`.
 */
function installDev(fetches: Fetch[]) {
  const calls: boolean[] = [];
  const urls: string[] = [];
  let fetched = 0;
  const dev: NonNullable<typeof globalThis.__inca_dev__> = {
    setLoadFailed: (failed) => calls.push(failed),
    compile: (params, body) => new AsyncFunction(...params, body),
    send(_method, paramsJson) {
      const { data } = JSON.parse(paramsJson) as {
        data: { id: string; name: string; data: unknown[] };
      };
      if (data.name === "fetchModule") urls.push(data.data[0] as string);
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
    urls,
    fetched: () => fetched,
    edit: () =>
      dev.receive?.(
        "vite",
        JSON.stringify({ type: "custom", event: "file-changed", data: { file: "/a.vue" } }),
      ),
    custom: (event: string) =>
      dev.receive?.("vite", JSON.stringify({ type: "custom", event, data: {} })),
    /** Sends the update of a module that accepts itself. */
    update: (path: string) =>
      dev.receive?.(
        "vite",
        JSON.stringify({
          type: "update",
          updates: [{ type: "js-update", path, acceptedPath: path, timestamp: 1 }],
        }),
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

  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  });

  afterEach(() => {
    vi.useRealTimers();
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
    await vi.advanceTimersByTimeAsync(SETTLE_MS);
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
    await vi.advanceTimersByTimeAsync(SETTLE_MS);
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
    await vi.advanceTimersByTimeAsync(SETTLE_MS);
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
    await vi.advanceTimersByTimeAsync(SETTLE_MS);
    await vi.waitFor(() => expect(dev.calls).toEqual([true, false, true]));
    dev.edit();
    await vi.advanceTimersByTimeAsync(SETTLE_MS);
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
    await vi.advanceTimersByTimeAsync(SETTLE_MS);
    await started;

    expect(dev.calls).toEqual([true, false]);
  });

  it("restarts the wait when a payload arrives during it", async () => {
    const dev = installDev([
      () => {
        throw new Error("compile error");
      },
      () => module("return 1"),
    ]);

    const started = start("/entry.js");
    await vi.waitFor(() => expect(dev.calls).toEqual([true]));
    dev.edit();
    await vi.advanceTimersByTimeAsync(SETTLE_MS - 1);
    dev.edit();
    await vi.advanceTimersByTimeAsync(SETTLE_MS);
    expect(dev.fetched()).toBe(1);
    await vi.advanceTimersByTimeAsync(1);
    await started;

    expect(dev.fetched()).toBe(2);
  });

  it("clears the failed flag SETTLE_MS after the payload", async () => {
    const dev = installDev([
      () => {
        throw new Error("compile error");
      },
      () => module("return 1"),
    ]);

    const started = start("/entry.js");
    await vi.waitFor(() => expect(dev.calls).toEqual([true]));
    dev.edit();
    await vi.advanceTimersByTimeAsync(SETTLE_MS - 1);
    expect(dev.calls).toEqual([true]);
    await vi.advanceTimersByTimeAsync(1);
    await started;

    expect(dev.calls).toEqual([true, false]);
  });

  it("applies an update that arrived before its module accepted, once the import ends", async () => {
    const log = captureLog();
    const inner = gated();
    const dev = installDev([
      () => module(importing("/dep.js")),
      () => module(`await __vite_ssr_import__("/inner.js"); ${accepting("dep v1")}`, "/dep.js"),
      async () => {
        await inner.gate;
        return module("", "/inner.js");
      },
      () => module(accepting("dep v2"), "/dep.js", true),
    ]);

    const started = start("/entry.js");
    await vi.waitFor(() => expect(dev.fetched()).toBe(3));
    dev.update("/dep.js");
    inner.release();
    await started;

    await vi.waitFor(() => expect(log).toEqual(["dep v1", "entry", "dep v2"]));
    expect(dev.urls).toEqual(["/entry.js", "/dep.js", "/inner.js", "/dep.js"]);
  });

  it("re-runs only the accept of a module that loaded after its update", async () => {
    const log = captureLog();
    const entry = gated();
    const dev = installDev([
      async () => {
        await entry.gate;
        return module(importing("/dep.js"));
      },
      () => module(accepting("dep v2"), "/dep.js"),
      () => ({ cache: true }),
    ]);

    const started = start("/entry.js");
    await vi.waitFor(() => expect(dev.fetched()).toBe(1));
    dev.update("/dep.js");
    entry.release();
    await started;

    await vi.waitFor(() => expect(dev.fetched()).toBe(3));
    expect(dev.urls).toEqual(["/entry.js", "/dep.js", "/dep.js"]);
    expect(log).toEqual(["dep v2", "entry"]);
  });

  it("drops the updates of a failed import and retries with the current modules", async () => {
    const log = captureLog();
    const bad = gated();
    const dev = installDev([
      () => module(importing("/dep.js", "/bad.js")),
      () => module(accepting("dep v1"), "/dep.js"),
      async () => {
        await bad.gate;
        throw new Error("compile error");
      },
      () => module(importing("/dep.js", "/bad.js")),
      () => module(accepting("dep v2"), "/dep.js"),
      () => module("", "/bad.js"),
    ]);

    const started = start("/entry.js");
    await vi.waitFor(() => expect(dev.fetched()).toBe(3));
    dev.update("/dep.js");
    dev.edit();
    bad.release();
    await vi.waitFor(() => expect(dev.calls).toEqual([true]));
    await vi.advanceTimersByTimeAsync(SETTLE_MS);
    await started;

    expect(log).toEqual(["dep v1", "dep v2", "entry"]);
    expect(dev.urls).toEqual([
      "/entry.js",
      "/dep.js",
      "/bad.js",
      "/entry.js",
      "/dep.js",
      "/bad.js",
    ]);
  });

  it("passes a custom event that a module waits on during the import", async () => {
    const log = captureLog();
    const dev = installDev([
      () => module(importing("/dep.js")),
      () =>
        module(
          `globalThis.__log.push("dep waits"); await new Promise((resolve) => __vite_ssr_import_meta__.hot.on("x:reply", resolve)); ${accepting("dep v1")}`,
          "/dep.js",
        ),
      () => module(accepting("dep v2"), "/dep.js", true),
    ]);

    const started = start("/entry.js");
    await vi.waitFor(() => expect(log).toEqual(["dep waits"]));
    dev.update("/dep.js");
    dev.custom("x:reply");

    await vi.waitFor(() => expect(log).toEqual(["dep waits", "dep v1", "entry", "dep v2"]));
    await started;
    expect(dev.urls).toEqual(["/entry.js", "/dep.js", "/dep.js"]);
  });

  it("applies the updates that arrived during the import in their order", async () => {
    const log = captureLog();
    const inner = gated();
    const dev = installDev([
      () => module(importing("/a.js", "/b.js", "/inner.js")),
      () => module(accepting("a v1"), "/a.js"),
      () => module(accepting("b v1"), "/b.js"),
      async () => {
        await inner.gate;
        return module("", "/inner.js");
      },
      () => module(accepting("b v2"), "/b.js", true),
      () => module(accepting("a v2"), "/a.js", true),
    ]);

    const started = start("/entry.js");
    await vi.waitFor(() => expect(dev.fetched()).toBe(4));
    dev.update("/b.js");
    dev.update("/a.js");
    inner.release();
    await started;

    await vi.waitFor(() => expect(log).toEqual(["a v1", "b v1", "entry", "b v2", "a v2"]));
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
    await vi.advanceTimersByTimeAsync(SETTLE_MS);

    await expect(started).resolves.toMatchObject({ answer: 42 });
    expect(rejected).toEqual([]);
  });

  it("sets the globals before the entry evaluates", async () => {
    installDev([() => module("__vite_ssr_exports__.seen = globalThis.__APP_CONFIG__;")]);
    onTestFinished(() => {
      delete (globalThis as { __APP_CONFIG__?: unknown }).__APP_CONFIG__;
    });

    await expect(start("/entry.js", { __APP_CONFIG__: true })).resolves.toMatchObject({
      seen: true,
    });
  });
});
