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
export { createIncaElement } from "./element/handle.mts";
export { isDestroyed } from "./element/lifecycle.mts";
/** @internal */
export { releaseWhenCollected } from "./element/lifecycle.mts";
export {
  createCommentNode,
  createElementNode,
  createTextNode,
  insert,
  nextSibling,
  parentNode,
  remove,
  setElementText,
  setText,
} from "./element/shadowTree.mts";
export type * from "./types.mts";
export type * from "./element/types.mts";

/**
 * What a custom renderer (`incajs/vue`) drives the host through: the tree and
 * event calls, the element handles and the shadow tree.
 */
export type IncaCore = typeof import("./rendererCore.mts");
