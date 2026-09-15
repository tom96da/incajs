// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { build, watch } from "./adapter/vite/index.mts";
import type { Bundler } from "./adapter/types.mts";

/**
 * `adapter/vite` wired in as the one adapter `dev` and `build` share by
 * default — a single place to swap bundlers, and the reason they can't
 * drift onto different ones.
 */
export const defaultBundler: Bundler = { watch, build };
