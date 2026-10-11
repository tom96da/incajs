// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { layoutBox } from "./layoutBox.mts";
import { offsetParentOf } from "./offsetParent.mts";
import type { IncaCore } from "../rendererCore.mts";
import type { IncaElement } from "./nodeOps.mts";

/**
 * Where a node sits inside the padding edge of its offset parent, rounded to
 * whole pixels. A node with no offset parent is measured from the window
 * origin.
 * @param core - the incajs bindings to read through
 * @param el - the node to measure
 * @returns `left` and `top`; both `0` when the node has no recorded box
 */
export function offsetOf(core: IncaCore, el: IncaElement): { left: number; top: number } {
  const box = layoutBox(core, el);
  if (!box) return { left: 0, top: 0 };

  const parent = offsetParentOf(core, el);
  const origin = parent ? layoutBox(core, parent) : null;
  // The box moved with the scroll its parent applied when it was recorded.
  return {
    left: Math.round(box[0] - (origin ? origin[0] + origin[4] : 0) + box[5]) || 0,
    top: Math.round(box[1] - (origin ? origin[1] + origin[4] : 0) + box[6]) || 0,
  };
}
