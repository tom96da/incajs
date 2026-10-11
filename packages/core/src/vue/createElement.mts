// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { isDestroyed } from "./destroyedNodes.mts";
import { domRect } from "./domRect.mts";
import { elements } from "./elements.mts";
import { layoutBox } from "./layoutBox.mts";
import { offsetOf } from "./offsetOf.mts";
import { offsetParentOf } from "./offsetParent.mts";
import { scrollPosition } from "./scrollPosition.mts";
import { skipReactive } from "./skipReactive.mts";
import { tabIndexOf } from "./tabIndexOf.mts";
import type { IncaCore, NodeId, TagName } from "../rendererCore.mts";
import type { IncaElement } from "./nodeOps.mts";
import type { ScrollArgs } from "./scrollPosition.mts";

/**
 * Builds the {@link IncaElement} handle for a native node: parentless,
 * childless, with `focus()`, `blur()`, `click()`, the listener methods of an
 * event target, `tabIndex` and the layout members, all driven through `core`.
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
  const scroll = () => (isDestroyed(element) ? null : core.scrollOf(id));
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
    getBoundingClientRect: () => domRect(layoutBox(core, element)),
    get offsetWidth() {
      return Math.round(layoutBox(core, element)?.[2] ?? 0);
    },
    get offsetHeight() {
      return Math.round(layoutBox(core, element)?.[3] ?? 0);
    },
    get offsetLeft() {
      return offsetOf(core, element).left;
    },
    get offsetTop() {
      return offsetOf(core, element).top;
    },
    get offsetParent() {
      return offsetParentOf(core, element);
    },
    get clientWidth() {
      const box = layoutBox(core, element);
      return box ? Math.max(0, Math.round(box[2] - 2 * box[4])) : 0;
    },
    get clientHeight() {
      const box = layoutBox(core, element);
      return box ? Math.max(0, Math.round(box[3] - 2 * box[4])) : 0;
    },
    get clientLeft() {
      return Math.round(layoutBox(core, element)?.[4] ?? 0);
    },
    get clientTop() {
      return Math.round(layoutBox(core, element)?.[4] ?? 0);
    },
    get scrollLeft() {
      return scroll()?.[0] ?? 0;
    },
    set scrollLeft(value: number) {
      scrollPosition(core, element, [value], false);
    },
    get scrollTop() {
      return scroll()?.[1] ?? 0;
    },
    set scrollTop(value: number) {
      scrollPosition(core, element, [undefined, value], false);
    },
    get scrollWidth() {
      return element.clientWidth + Math.round(scroll()?.[2] ?? 0);
    },
    get scrollHeight() {
      return element.clientHeight + Math.round(scroll()?.[3] ?? 0);
    },
    scrollTo: (...args: ScrollArgs) => scrollPosition(core, element, args, false),
    scrollBy: (...args: ScrollArgs) => scrollPosition(core, element, args, true),
  };
  if (kind === "element") elements.set(id, element);
  // A plain object in `ref()` becomes a proxy, which differs from `activeElement()`.
  return skipReactive(element);
}
