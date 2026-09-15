// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { configDefaults, defineConfig } from "vitest/config";

const exclude = [...configDefaults.exclude, "**/third_party/**", "**/target/**"];

export default defineConfig({
  test: {
    projects: ["packages/*"],
    exclude,
    coverage: { provider: "v8", exclude: [...exclude, "**/tests/**"] },
  },
});
