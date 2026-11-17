// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import "./abortSignal.mts";
import "./targets.mts";

export { createEventTarget } from "./eventTarget.mts";
export { window } from "./window.mts";
export { addEventListener, removeEventListener, latestEventId } from "./events.mts";
export {
  focus,
  blur,
  dispatchEvent,
  click,
  focusedNode,
  createNode,
  holdNode,
  releaseNode,
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
} from "./ffi/index.mts";
export {
  createCommentNode,
  createElementNode,
  createIncaElement,
  createTextNode,
  rootElement,
  setElementText,
} from "./element/handle.mts";
export { isRemoved } from "./element/lifecycle.mts";
/** @internal */
export { releaseWhenCollected } from "./element/lifecycle.mts";
export { insert, nextSibling, parentNode, remove } from "./element/treeOps.mts";
export { setText } from "./element/text.mts";
export type * from "./types.mts";
export type { HandleNode } from "./element/handleNode.mts";
export type * from "./element/types.mts";
export type * from "./element/nodeTypes.mts";

/**
 * What a custom renderer (`incajs/vue`) drives the host through: the tree and
 * event calls, the element handles and the tree operations.
 */
export type RendererCore = typeof import("./rendererCore.mts");
