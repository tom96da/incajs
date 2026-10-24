// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { defineConfig } from "oxlint";
import type { OxlintOverride } from "oxlint";

import rootConfig from "../../oxlint.config.ts";

const overrides: OxlintOverride[] = [
  {
    files: ["src/element/**/*.mts", "src/*.mts"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: ["@vue/*", "**/vue/**"],
              message: "Core features are framework-agnostic. Keep Vue code in the renderer.",
            },
            {
              group: ["**/rendererCore.mts"],
              message: "Core features do not use the injected core. Import the defining module.",
            },
          ],
        },
      ],
    },
  },
  {
    files: ["src/vue/createApp.mts", "src/vue/**/*.test.mts"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: ["../**", "!../rendererCore.mts", "incajs", "incajs/**"],
              message: "The renderer uses core features only through the injected core.",
            },
          ],
        },
      ],
    },
  },
  {
    files: ["src/vue/**/*.mts"],
    excludeFiles: ["src/vue/createApp.mts", "src/vue/**/*.test.mts"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: ["../**", "!../rendererCore.mts", "incajs", "incajs/**"],
              message: "The renderer uses core features only through the injected core.",
            },
          ],
          paths: [
            {
              name: "../rendererCore.mts",
              allowTypeImports: true,
              message:
                "Only the composition root imports the injected core as a value. Others take it as a parameter.",
            },
          ],
        },
      ],
    },
  },
  {
    files: ["src/**/*.mts"],
    excludeFiles: ["src/vue/runtime.mts", "src/**/*.test.mts"],
    rules: {
      "import/no-nodejs-modules": "error",
    },
  },
];

export default defineConfig({
  extends: [rootConfig],
  overrides,
});
