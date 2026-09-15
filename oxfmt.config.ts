// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { defineConfig } from "oxfmt";

export default defineConfig({
  ignorePatterns: ["**.md", "third_party/**"],
  sortImports: {
    order: "asc",
    ignoreCase: true,
    newlinesBetween: true,
    customGroups: [
      {
        groupName: "first-party",
        elementNamePattern: ["incajs*"],
        modifiers: ["value"],
      },
      {
        groupName: "type-first-party",
        elementNamePattern: ["incajs*"],
        modifiers: ["type"],
      },
      {
        groupName: "scoped-first-party",
        elementNamePattern: ["@incajs/*"],
        modifiers: ["value"],
      },
      {
        groupName: "type-scoped-first-party",
        elementNamePattern: ["@incajs/*"],
        modifiers: ["type"],
      },
    ],
    groups: [
      "builtin",
      { newlinesBetween: false },
      "type-builtin",

      "external",
      { newlinesBetween: false },
      "type-external",

      "first-party",
      { newlinesBetween: false },
      "type-first-party",

      "scoped-first-party",
      { newlinesBetween: false },
      "type-scoped-first-party",

      ["internal", "subpath"],
      { newlinesBetween: false },
      ["type-internal", "type-subpath"],

      ["parent", "sibling", "index"],
      { newlinesBetween: false },
      ["type-parent", "type-sibling", "type-index"],

      "style",
      "unknown",
    ],
  },
});
