// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

export { createEventTarget } from "./createEventTarget.mts";
export {
  addEventListener,
  removeEventListener,
  latestEventId,
  focus,
  blur,
  focusedNode,
  dispatchEvent,
  click,
} from "./events.mts";
export {
  appendChild,
  createNode,
  destroyNode,
  insertBefore,
  removeChild,
  rootNodeId,
  setAttribute,
  getAttribute,
  layoutOf,
  scrollOf,
  scrollNode,
  removeAttribute,
  setStyle,
  removeStyle,
} from "./tree.mts";
export type * from "./types.mts";

/**
 * The tree/event primitives a custom renderer (`incajs/vue`) drives the
 * native tree through.
 *
 * Also the seam a renderer's own tests mock against, in place of a real
 * `globalThis.__inca_native__`.
 */
export type IncaCore = typeof import("./rendererCore.mts");
