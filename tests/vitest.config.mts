// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["tests/*.test.mts"],
    setupFiles: ["./support/headless.mts"],
  },
});
