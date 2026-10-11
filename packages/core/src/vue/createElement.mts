// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { isDestroyed } from "./destroyedNodes.mts";
import { elements } from "./elements.mts";
import { skipReactive } from "./skipReactive.mts";
import { tabIndexOf } from "./tabIndexOf.mts";
import type { IncaCore, NodeId, TagName } from "../rendererCore.mts";
import type { IncaElement } from "./nodeOps.mts";

/**
 * Builds the {@link IncaElement} handle for a native node: parentless,
 * childless, with `focus()`, `blur()`, `click()`, the listener methods of an
 * event target and `tabIndex`, all driven through `core`.
 * @param core - the incajs bindings the handle drives
 * @param id - the native node's id
 * @param kind - whether the node is an element or a comment stand-in
 * @param tag - the node's element kind, if it has one; a `button` defaults to tab index `0`
 * @returns the new handle
 */
export function createIncaElement(
  core: IncaCore,
  id: NodeId,
  kind: IncaElement["kind"],
  tag?: TagName,
): IncaElement {
  const target = core.createEventTarget(() => id);
  const element: IncaElement = {
    id,
    kind,
    parent: null,
    children: [],
    focus: () => core.focus(id),
    blur: () => core.blur(id),
    click: () => core.click(id),
    addEventListener: (...args) => {
      if (!isDestroyed(element)) target.addEventListener(...args);
    },
    removeEventListener: (...args) => {
      if (!isDestroyed(element)) target.removeEventListener(...args);
    },
    dispatchEvent: (event) => isDestroyed(element) || target.dispatchEvent(event),
    get tabIndex() {
      return tabIndexOf(isDestroyed(element) ? null : core.getAttribute(id, "tabindex"), tag ?? "");
    },
    set tabIndex(value: number) {
      if (!isDestroyed(element)) core.setAttribute(id, "tabindex", value | 0);
    },
  };
  if (kind === "element") elements.set(id, element);
  // A plain object in `ref()` becomes a proxy, which differs from `activeElement()`.
  return skipReactive(element);
}
