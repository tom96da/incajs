// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The members that act on an element: focus, click and the pointer capture.

import {
  blur,
  click,
  focus,
  getAttribute,
  hasPointerCapture,
  HTML_NAMESPACE,
  releasePointerCapture,
  setAttribute,
  setPointerCapture,
} from "../ffi/index.mts";
import { tabIndexOf } from "./geometry.mts";
import {
  ELEMENT_RULE,
  HTML_ELEMENT_RULE,
  HTML_OR_SVG_RULE,
  isDestroyed,
  isRemoved,
  receiverNode,
} from "./lifecycle.mts";
import type { Receiver } from "./lifecycle.mts";
import type { Element, HTMLElement } from "./types.mts";

/**
 * Reads the receiver of an element member.
 * @param self - `this` of the member
 * @param rule - the receiver rule of the member
 * @returns the element
 */
function elementReceiver(self: unknown, rule: Receiver): Element {
  return receiverNode<Element>(self, rule);
}

/** The members {@link pointerCaptureMembers} builds. */
export type PointerCaptureMembers = Pick<
  Element,
  "setPointerCapture" | "releasePointerCapture" | "hasPointerCapture"
>;

/**
 * Builds the pointer capture members of `Element`.
 * @returns the members
 */
export function pointerCaptureMembers(): PointerCaptureMembers {
  return {
    setPointerCapture(pointerId: number) {
      setPointerCapture(elementReceiver(this, ELEMENT_RULE).__node_id, pointerId);
    },
    releasePointerCapture(pointerId: number) {
      releasePointerCapture(elementReceiver(this, ELEMENT_RULE).__node_id, pointerId);
    },
    hasPointerCapture(pointerId: number) {
      return hasPointerCapture(elementReceiver(this, ELEMENT_RULE).__node_id, pointerId);
    },
  };
}

/** The members {@link focusMembers} builds. */
export type FocusMembers = Pick<HTMLElement, "focus" | "blur" | "tabIndex">;

/**
 * Builds `focus()`, `blur()` and `tabIndex` of the shared element members.
 * @returns the members
 */
export function focusMembers(): FocusMembers {
  return {
    focus(...rest: [options?: { preventScroll?: boolean }]) {
      const element = elementReceiver(this, HTML_OR_SVG_RULE);
      if (!isRemoved(element)) focus(element.__node_id, Boolean(rest[0]?.preventScroll));
    },
    blur() {
      blur(elementReceiver(this, HTML_OR_SVG_RULE).__node_id);
    },
    get tabIndex() {
      const element = elementReceiver(this, HTML_OR_SVG_RULE);
      return tabIndexOf(
        isDestroyed(element) ? null : getAttribute(element.__node_id, "tabindex"),
        element.namespaceURI === HTML_NAMESPACE ? element.localName : "",
      );
    },
    set tabIndex(value: number) {
      const element = elementReceiver(this, HTML_OR_SVG_RULE);
      if (!isDestroyed(element)) setAttribute(element.__node_id, "tabindex", value | 0);
    },
  };
}

/**
 * Builds `click()` of `HTMLElement`.
 * @returns the members
 */
export function clickMembers(): Pick<HTMLElement, "click"> {
  return {
    click() {
      const element = elementReceiver(this, HTML_ELEMENT_RULE);
      if (!isRemoved(element)) click(element.__node_id);
    },
  };
}
