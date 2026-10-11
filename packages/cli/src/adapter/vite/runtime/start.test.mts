// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";

import { start } from "./start.mts";

type Fetch = () => unknown;

/** Stands in for the host's `compile`; it ignores the file name. */
const AsyncFunction = async function () {}.constructor as new (
  ...args: string[]
) => (...args: unknown[]) => Promise<unknown>;

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
    compile: (params, body) => new AsyncFunction(...params, body),
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
