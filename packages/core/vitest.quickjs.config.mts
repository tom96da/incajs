// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/** Runs the unit tests in QuickJS with `vitest-pool-quickjs`: `pnpm -F incajs test:quickjs`. */

import { quickjsPool } from "vitest-pool-quickjs";
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    pool: quickjsPool({
      externals: [
        "node:v8",
        "node:vm",
        "@vue/runtime-core/dist/runtime-core.cjs.js",
        "@vue/runtime-core/dist/runtime-core.cjs.prod.js",
        "@vue/runtime-dom/dist/runtime-dom.cjs.js",
      ],
    }),
    // the context has no file system to read the cache of Vitest from
    fsModuleCache: false,
    setupFiles: ["./tests/setup.mts"],
    include: ["src/**/*.test.mts", "tests/*.test.mts"],
  },
});
