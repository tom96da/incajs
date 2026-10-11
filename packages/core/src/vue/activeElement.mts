// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import * as core from "../rendererCore.mts";
import { elements } from "./elements.mts";
import type { IncaElement } from "./nodeOps.mts";

/**
 * Returns the element that currently has focus. A `focus()` or `blur()` call
 * takes effect for this function at once.
 * @returns the focused element, or `null` when no element has focus
 */
export function activeElement(): IncaElement | null {
  const id = core.focusedNode();
  return id === null ? null : (elements.get(id) ?? null);
}
