// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import "./targets.mts";

export { createEventTarget } from "./eventTarget.mts";
export { window } from "./window.mts";
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
export { insert, nextSibling, parentNode, remove } from "./element/shadowTree.mts";
export { setText } from "./element/text.mts";
export type * from "./types.mts";
export type * from "./element/types.mts";

/**
 * What a custom renderer (`incajs/vue`) drives the host through: the tree and
 * event calls, the element handles and the shadow tree.
 */
export type IncaCore = typeof import("./rendererCore.mts");
