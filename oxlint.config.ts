// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { defineConfig } from "oxlint";

export default defineConfig({
  plugins: ["eslint", "typescript", "unicorn", "oxc", "import", "node", "jsdoc", "vitest", "vue"],
  jsPlugins: ["eslint-plugin-check-file"],
  ignorePatterns: ["third_party/**"],
  rules: {
    "check-file/folder-naming-convention": ["error", { "packages/*/src/**/": "KEBAB_CASE" }],
    "check-file/filename-naming-convention": [
      "error",
      { "packages/*/src/**/*.mts": "CAMEL_CASE" },
      { ignoreMiddleExtensions: true },
    ],
    "vue/prefer-import-from-vue": "off",
  },
});
