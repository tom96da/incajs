// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import * as adapterCore from "./adapterCore.mts";
import { createViteBundler } from "./vite/index.mts";
import type { Bundler } from "./types.mts";

/**
 * `vite/` wired in as the one adapter `dev` and `build` share by
 * default: a single place to swap bundlers. Compiled `.vue` templates import
 * their helpers from `incajs/vue/runtime`.
 */
export const defaultBundler: Bundler = createViteBundler(adapterCore, "incajs/vue/runtime");
