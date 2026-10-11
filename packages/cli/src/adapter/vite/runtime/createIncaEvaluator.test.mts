// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import {
  type EvaluatedModuleNode,
  type ModuleRunnerContext,
  ssrDynamicImportKey,
  ssrExportAllKey,
  ssrExportNameKey,
  ssrImportKey,
  ssrImportMetaKey,
  ssrModuleExportsKey,
} from "vite/module-runner";
import { afterEach, describe, expect, it } from "vitest";

import { createIncaEvaluator } from "./createIncaEvaluator.mts";

const KEYS = [
  ssrModuleExportsKey,
  ssrImportMetaKey,
  ssrImportKey,
  ssrDynamicImportKey,
  ssrExportAllKey,
  ssrExportNameKey,
];

const module = {
  id: "/src/App.vue",
  url: "/src/App.vue?t=1",
} as unknown as Readonly<EvaluatedModuleNode>;

/** A context whose values are their own key names, except the exports object. */
function contextWith(exports: object): ModuleRunnerContext {
  return Object.fromEntries(
    KEYS.map((key) => [key, key === ssrModuleExportsKey ? exports : key]),
  ) as unknown as ModuleRunnerContext;
}

describe("createIncaEvaluator", () => {
  afterEach(() => {
    globalThis.__inca_dev__ = undefined;
  });

  it("has a startOffset of 1", () => {
    expect(createIncaEvaluator().startOffset).toBe(1);
  });

  it("compiles the code with the six context keys, named by the module id", async () => {
    const compiled: { params: string[]; body: string; filename: string }[] = [];
    globalThis.__inca_dev__ = {
      send() {},
      compile(params, body, filename) {
        compiled.push({ params, body, filename });
        return async () => {};
      },
    };

    await createIncaEvaluator().runInlinedModule(contextWith({}), "const a = 1;", module);

    expect(compiled).toEqual([{ params: KEYS, body: "const a = 1;", filename: "/src/App.vue" }]);
  });

  it("escapes each NUL byte in the code and the id as \\u0000", async () => {
    const compiled: { body: string; filename: string }[] = [];
    globalThis.__inca_dev__ = {
      send() {},
      compile(_params, body, filename) {
        compiled.push({ body, filename });
        return async () => {};
      },
    };
    const virtual = {
      id: "\0virtual",
      url: "\0virtual",
    } as unknown as Readonly<EvaluatedModuleNode>;

    await createIncaEvaluator().runInlinedModule(contextWith({}), "a\0b\0", virtual);

    expect(compiled).toEqual([{ body: "a\\u0000b\\u0000", filename: "\\u0000virtual" }]);
  });

  it("calls the compiled function with the six context values in order", async () => {
    let args: unknown[] = [];
    globalThis.__inca_dev__ = {
      send() {},
      compile:
        () =>
        async (...received) => {
          args = received;
        },
    };

    await createIncaEvaluator().runInlinedModule(contextWith({}), "", module);

    expect(args).toEqual(KEYS.map((key) => (key === ssrModuleExportsKey ? {} : key)));
  });

  it("seals the exports object once the module has run", async () => {
    globalThis.__inca_dev__ = {
      send() {},
      compile: () => async () => {},
    };
    const exports = {};

    await createIncaEvaluator().runInlinedModule(contextWith(exports), "", module);

    expect(Object.isSealed(exports)).toBe(true);
  });

  it("rejects when __inca_dev__ or its compile is missing", async () => {
    for (const dev of [undefined, { send() {} }]) {
      globalThis.__inca_dev__ = dev;
      await expect(
        createIncaEvaluator().runInlinedModule(contextWith({}), "", module),
      ).rejects.toThrow("__inca_dev__.compile is not installed");
    }
  });
});
