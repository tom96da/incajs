// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { layoutBox } from "./layoutBox.mts";
import type { IncaCore } from "../rendererCore.mts";
import type { IncaElement } from "./nodeOps.mts";

/**
 * The parent a node's offsets are measured from. It is the parent when both
 * the node and the parent have a recorded box.
 * @param core - the incajs bindings to read through
 * @param el - the node to query
 * @returns the parent, or `null`
 */
export function offsetParentOf(core: IncaCore, el: IncaElement): IncaElement | null {
  const parent = el.parent;
  if (!layoutBox(core, el) || !parent || !layoutBox(core, parent)) return null;
  return parent;
}
