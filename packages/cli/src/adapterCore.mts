// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

export { IncaError } from "./error.mts";
export { withAppRoot } from "./config/appRoot.mts";
export { bundlerFault } from "./log.mts";
export type * from "./types.mts";

/**
 * The CLI logic the CLI injects into a bundler adapter: the `IncaError`
 * class, `bundlerFault`, which reduces a bundler's error to a printable
 * fault, and `withAppRoot`, which records the app's root in a runtime config.
 *
 * Also the seam an adapter's own tests pass the real module through.
 */
export type AdapterCore = typeof import("./adapterCore.mts");
