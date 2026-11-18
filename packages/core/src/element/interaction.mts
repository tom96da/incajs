// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The `tabIndex` member of the elements that take focus.

import { getAttribute, HTML_NAMESPACE, setAttribute } from "../ffi/index.mts";
import { tabIndexOf } from "./geometry.mts";
import { HTML_OR_SVG_RULE, isDestroyed, receiverNode } from "./lifecycle.mts";
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

/** The members {@link focusMembers} builds. */
export type FocusMembers = Pick<HTMLElement, "tabIndex">;

/**
 * Builds `tabIndex` of the shared element members. The host defines `focus()` and `blur()`.
 * @returns the members
 */
export function focusMembers(): FocusMembers {
  return {
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
