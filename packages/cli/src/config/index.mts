// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { createDefineConfig } from "c12";

import type { IncaConfig } from "./types.mts";

export { defaultConfig } from "./defaults.mts";
export type { IncaConfig } from "./types.mts";

/**
 * Type-completion helper for `inca.config.ts` — an identity function at
 * runtime, the same way `defineConfig` from `vite` or `vitest/config` works.
 *
 * @example
 * ```ts
 * import { defineConfig } from "@incajs/cli/config";
 *
 * export default defineConfig({
 *   productName: "Click Counter",
 *   identifier: "com.example.click-counter",
 * });
 * ```
 */
export const defineConfig = createDefineConfig<IncaConfig>();
