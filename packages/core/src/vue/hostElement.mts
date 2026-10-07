// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { IncaCore } from "../rendererCore.mts";
import type { NodeId, TagName } from "../types.mts";
import type { IncaElement } from "./nodeOps.mts";

/**
 * Builds the {@link IncaElement} handle for a native node: parentless,
 * childless, with `focus()`, `blur()` and `tabIndex` wired to `core`.
 * @param core - the incajs bindings the handle drives
 * @param id - the native node's id
 * @param kind - whether the node is an element or a comment stand-in
 * @param tag - the node's element kind, if it has one; a `button` starts at tab index `0`
 * @returns the new handle
 */
export function hostElement(
  core: IncaCore,
  id: NodeId,
  kind: IncaElement["kind"],
  tag?: TagName,
): IncaElement {
  let tabIndex = tag === "button" ? 0 : -1;
  return {
    id,
    kind,
    parent: null,
    children: [],
    focus: () => core.focus(id),
    blur: () => core.blur(id),
    get tabIndex() {
      return tabIndex;
    },
    set tabIndex(value: number) {
      tabIndex = Math.trunc(value);
      core.setAttribute(id, "tabindex", tabIndex);
    },
  };
}
