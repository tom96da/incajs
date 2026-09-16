// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/**
 * The subset of `incajs`'s own API a custom renderer (`incajs/vue`, and any
 * future framework adapter alongside it) drives the native tree through —
 * derived from `index.mts`'s real exports, so this contract can't drift out
 * of sync with them.
 *
 * Also the seam a renderer's own tests mock against, in place of a real
 * `globalThis.__inca_native__`.
 */
export type IncaCore = Pick<
  typeof import("./index.mts"),
  | "rootNodeId"
  | "createNode"
  | "appendChild"
  | "insertBefore"
  | "removeChild"
  | "destroyNode"
  | "setAttribute"
  | "setStyle"
  | "setEventListener"
  | "removeEventListener"
>;
