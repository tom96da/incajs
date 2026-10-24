// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { defineConfig } from "oxlint";
import type { OxlintOverride } from "oxlint";

import rootConfig from "../../oxlint.config.ts";

const overrides: OxlintOverride[] = [
  {
    files: ["src/**/*.mts"],
    excludeFiles: ["src/vite/**", "src/defaultBundler.mts", "src/**/*.test.mts"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: ["vite", "vite/**", "@vitejs/**"],
              message: "CLI features are bundler-agnostic. Keep Vite code in the Vite adapter.",
            },
            {
              group: ["**/vite/**"],
              message: "CLI features do not use the Vite adapter. Use the Bundler contract.",
            },
          ],
        },
      ],
    },
  },
  {
    files: ["src/defaultBundler.mts", "src/**/*.test.mts"],
    excludeFiles: ["src/vite/**"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: ["vite", "vite/**", "@vitejs/**"],
              message: "CLI features are bundler-agnostic. Keep Vite code in the Vite adapter.",
            },
          ],
        },
      ],
    },
  },
  {
    files: ["src/vite/*.mts"],
    excludeFiles: ["src/vite/*.test.mts"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: ["../**", "!../adapterCore.mts"],
              message: "The adapter uses CLI features only through the injected adapter core.",
            },
          ],
          paths: [
            {
              name: "../adapterCore.mts",
              allowTypeImports: true,
              message:
                "Only the composition root imports the adapter core as a value. Others take it as a parameter.",
            },
          ],
        },
      ],
    },
  },
  {
    files: ["src/vite/runtime/**/*.mts"],
    excludeFiles: ["src/vite/runtime/**/*.test.mts"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: ["../../**"],
              message: "The runtime bundle imports no CLI code.",
            },
          ],
        },
      ],
      "import/no-nodejs-modules": "error",
    },
  },
];

export default defineConfig({
  extends: [rootConfig],
  overrides,
});
