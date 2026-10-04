// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

export { IncaError } from "./error.mts";
export { bundlerFault } from "./log.mts";

/**
 * The CLI logic the CLI injects into a bundler adapter: the `IncaError`
 * class and `bundlerFault`, which reduces a bundler's error to a printable
 * fault.
 *
 * Also the seam an adapter's own tests pass the real module through.
 */
export type AdapterCore = typeof import("./adapterCore.mts");
