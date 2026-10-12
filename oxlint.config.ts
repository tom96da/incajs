// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { defineConfig } from "oxlint";

export default defineConfig({
  plugins: ["eslint", "typescript", "unicorn", "oxc", "import", "node", "jsdoc", "vitest", "vue"],
  jsPlugins: ["eslint-plugin-check-file"],
  ignorePatterns: ["third_party/**", "specs/**"],
  overrides: [
    {
      files: ["packages/core/src/element/**/*.mts", "packages/core/src/*.mts"],
      rules: {
        "no-restricted-imports": [
          "error",
          {
            patterns: [
              {
                group: ["@vue/*", "**/vue/**"],
                message: "The framework-independent layers cannot import Vue or src/vue.",
              },
            ],
          },
        ],
      },
    },
  ],
  rules: {
    "import/no-duplicates": "error",
    "check-file/folder-naming-convention": ["error", { "packages/*/src/**/": "KEBAB_CASE" }],
    "check-file/filename-naming-convention": [
      "error",
      { "packages/*/src/**/*.mts": "CAMEL_CASE" },
      { ignoreMiddleExtensions: true },
    ],
    "vue/prefer-import-from-vue": "off",
  },
});
