// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { isDestroyed } from "./destroyedNodes.mts";
import type { IncaCore } from "../rendererCore.mts";
import type { IncaElement } from "./nodeOps.mts";

/** `[x, y, width, height, border, scrollX, scrollY]` of a node's last drawn frame. */
export type LayoutBox = readonly [number, number, number, number, number, number, number];

/**
 * Reads the box a node was last drawn at. A destroyed node, or one with no
 * recorded box, yields `null`.
 * @param core - the incajs bindings to read through
 * @param el - the node to read
 * @returns the box, or `null`
 */
export function layoutBox(core: IncaCore, el: IncaElement): LayoutBox | null {
  return isDestroyed(el) ? null : core.layoutOf(el.id);
}
