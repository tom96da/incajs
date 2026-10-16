// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { elementOf } from "./element/lifecycle.mts";
import { rootNodeId } from "./tree.mts";
import { window } from "./window.mts";

/**
 * Gives the objects that events carry for node ids: `window` for the host
 * root, the element of a node that has one, and `null` for any other node.
 * The host calls it at the start of each walk that reaches a listener and on
 * every `dispatchEvent()`, and once more for an event's `relatedTarget` node.
 */
globalThis.__inca_targets__ ??= (nodeIds) => {
  const root = rootNodeId();
  return nodeIds.map((id) => (id === root ? window : (elementOf(id) ?? null)));
};
