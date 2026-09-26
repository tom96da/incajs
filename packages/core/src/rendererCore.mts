// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

export { setEventListener, removeEventListener, focus, blur } from "./events.mts";
export {
  appendChild,
  createNode,
  destroyNode,
  insertBefore,
  removeChild,
  rootNodeId,
  setAttribute,
  setStyle,
} from "./tree.mts";

/**
 * The subset of `incajs`'s own API a custom renderer (`incajs/vue`, and any
 * future framework adapter alongside it) drives the native tree through.
 *
 * Also the seam a renderer's own tests mock against, in place of a real
 * `globalThis.__inca_native__`.
 */
export type IncaCore = typeof import("./rendererCore.mts");
