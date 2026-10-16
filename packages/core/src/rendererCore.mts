// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import "./targets.mts";

export { createEventTarget } from "./eventTarget.mts";
export {
  addEventListener,
  removeEventListener,
  latestEventId,
  focus,
  blur,
  dispatchEvent,
  click,
} from "./events.mts";
export {
  focusedNode,
  appendChild,
  createNode,
  destroyNode,
  holdNode,
  releaseNode,
  insertBefore,
  removeChild,
  rootNodeId,
  setAttribute,
  getAttribute,
  layoutOf,
  scrollOf,
  scrollNode,
  scrollIntoViewNode,
  removeAttribute,
  setStyle,
  removeStyle,
} from "./tree.mts";
export type * from "./types.mts";

/**
 * The tree/event primitives a custom renderer (`incajs/vue`) drives the
 * native tree through.
 */
export type IncaCore = typeof import("./rendererCore.mts");
