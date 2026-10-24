// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import {
  blur,
  click,
  focus,
  hasPointerCapture,
  releasePointerCapture,
  setPointerCapture,
} from "../events.mts";
import { createEventTarget } from "../eventTarget.mts";
import {
  focusedNode,
  getAttribute,
  rootNodeId,
  scrollIntoViewNode,
  scrollOf,
  setAttribute,
} from "../tree.mts";
import { withEventHandlers } from "./eventHandlers.mts";
import { layoutBox, offsetOf, offsetParentOf, rectOf, tabIndexOf } from "./geometry.mts";
import { elementOf, isDestroyed, isNode, registerElement, withNodeId } from "./lifecycle.mts";
import { scrollIntoViewArgs, scrollPosition } from "./scroll.mts";
import type { NodeId, TagName } from "../types.mts";
import type { ScrollArgs, ScrollIntoViewOptions } from "./scroll.mts";
import type { EventHandlers, IncaElement, IncaNode } from "./types.mts";

/**
 * Builds the {@link IncaElement} handle for a native node: parentless,
 * childless, with `id`, `focus()`, `blur()`, `click()`, the pointer capture
 * methods, `contains()`, the listener methods of an event target, `tabIndex`,
 * the layout members and the event handler properties, all driven through the
 * host.
 * @param id - the native node's id
 * @param kind - whether the node is an element or a comment stand-in
 * @param tag - the node's element kind, if it has one; a `button` defaults to tab index `0`
 * @param passiveByDefault - event names whose listeners are passive unless their options say
 *   `passive: false`
 * @returns the new handle
 */
export function createIncaElement(
  id: NodeId,
  kind: IncaElement["kind"],
  tag?: TagName,
  passiveByDefault: readonly string[] = [],
): IncaElement {
  const target = createEventTarget(() => id, passiveByDefault);
  const scroll = () => (isDestroyed(element) ? null : scrollOf(id));
  const fields: Omit<IncaElement, "__node_id" | keyof EventHandlers> = {
    get id() {
      return isDestroyed(element) ? "" : String(getAttribute(id, "id") ?? "");
    },
    set id(value: string) {
      if (!isDestroyed(element)) setAttribute(id, "id", String(value));
    },
    kind,
    parent: null,
    children: [],
    focus: (options?: { preventScroll?: boolean }) => {
      if (!isDestroyed(element)) focus(id, Boolean(options?.preventScroll));
    },
    blur: () => blur(id),
    click: () => {
      if (!isDestroyed(element)) click(id);
    },
    setPointerCapture: (pointerId: number) => setPointerCapture(id, pointerId),
    releasePointerCapture: (pointerId: number) => releasePointerCapture(id, pointerId),
    hasPointerCapture: (pointerId: number) => hasPointerCapture(id, pointerId),
    contains: (other: unknown) => {
      if (other === null || other === undefined) return false;
      if (!isNode(other)) {
        throw new TypeError("the value is not a node. pass an element, a text node or null");
      }
      for (let node: IncaNode | null = other; node !== null; node = node.parent) {
        if (node === element) return true;
      }
      return false;
    },
    addEventListener: (...args: Parameters<typeof target.addEventListener>) => {
      if (!isDestroyed(element)) target.addEventListener(...args);
    },
    removeEventListener: (...args: Parameters<typeof target.removeEventListener>) => {
      if (!isDestroyed(element)) target.removeEventListener(...args);
    },
    dispatchEvent: (event) => isDestroyed(element) || target.dispatchEvent(event),
    get tabIndex() {
      return tabIndexOf(isDestroyed(element) ? null : getAttribute(id, "tabindex"), tag ?? "");
    },
    set tabIndex(value: number) {
      if (!isDestroyed(element)) setAttribute(id, "tabindex", value | 0);
    },
    getBoundingClientRect: () => rectOf(layoutBox(element)),
    get offsetWidth() {
      return Math.round(layoutBox(element)?.[2] ?? 0);
    },
    get offsetHeight() {
      return Math.round(layoutBox(element)?.[3] ?? 0);
    },
    get offsetLeft() {
      return offsetOf(element).left;
    },
    get offsetTop() {
      return offsetOf(element).top;
    },
    get offsetParent() {
      return offsetParentOf(element);
    },
    get clientWidth() {
      const box = layoutBox(element);
      return box ? Math.max(0, Math.round(box[2] - 2 * box[4])) : 0;
    },
    get clientHeight() {
      const box = layoutBox(element);
      return box ? Math.max(0, Math.round(box[3] - 2 * box[4])) : 0;
    },
    get clientLeft() {
      return Math.round(layoutBox(element)?.[4] ?? 0);
    },
    get clientTop() {
      return Math.round(layoutBox(element)?.[4] ?? 0);
    },
    get scrollLeft() {
      return scroll()?.[0] ?? 0;
    },
    set scrollLeft(value: number) {
      scrollPosition(element, [value], false);
    },
    get scrollTop() {
      return scroll()?.[1] ?? 0;
    },
    set scrollTop(value: number) {
      scrollPosition(element, [undefined, value], false);
    },
    get scrollWidth() {
      return element.clientWidth + Math.round(scroll()?.[2] ?? 0);
    },
    get scrollHeight() {
      return element.clientHeight + Math.round(scroll()?.[3] ?? 0);
    },
    scrollTo: (...args: ScrollArgs) => scrollPosition(element, args, false),
    scrollBy: (...args: ScrollArgs) => scrollPosition(element, args, true),
    scrollIntoView: (arg?: boolean | ScrollIntoViewOptions) => {
      if (isDestroyed(element)) return;
      const { block, inline } = scrollIntoViewArgs(arg);
      scrollIntoViewNode(id, block, inline);
    },
  };
  const element: IncaElement = withEventHandlers(withNodeId(fields, id), () =>
    isDestroyed(element),
  );
  if (kind === "element") registerElement(element);
  return element;
}

/**
 * Returns the element that currently has focus. A `focus()` or `blur()` call
 * takes effect for this function at once.
 * @returns the focused element, or `null` when no element has focus
 */
export function activeElement(): IncaElement | null {
  const id = focusedNode();
  return id === null ? null : (elementOf(id) ?? null);
}

/**
 * Returns the handle of the host's mount container, which stands between the app's elements and
 * `document` in an event path. A live handle is reused, so a mount and an event see one object.
 * A `wheel` listener on it is passive unless its options say `passive: false`.
 * @returns the mount container's handle
 */
export function rootElement(): IncaElement {
  const id = rootNodeId();
  return elementOf(id) ?? createIncaElement(id, "element", undefined, ["wheel"]);
}
