// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { classList as listOf, getAttribute, setAttribute } from "../ffi/index.mts";
import { attributeText } from "./arguments.mts";
import { isDestroyed, receiverOf, receiverNode, ELEMENT_RULE } from "./lifecycle.mts";
import type { DOMTokenList, Element } from "./types.mts";

/** The members {@link classMembers} builds. */
export type ClassMembers = Pick<Element, "className" | "classList">;

/**
 * Builds `className` and `classList` of `Element`. The list is created on the first read and is
 * the same object afterwards.
 * @returns an object that holds the two members
 */
export function classMembers(): ClassMembers {
  const members = {
    get className() {
      const node = receiverNode(this, ELEMENT_RULE);
      return isDestroyed(node) ? "" : String(getAttribute(node.__node_id, "class") ?? "");
    },
    set className(value: string) {
      const node = receiverNode(this, ELEMENT_RULE);
      const text = attributeText(value);
      if (!isDestroyed(node)) setAttribute(node.__node_id, "class", text);
    },
    get classList(): DOMTokenList {
      const state = receiverOf(this, ELEMENT_RULE);
      return (state.classList ??= listOf((this as unknown as Element).__node_id));
    },
    set classList(value: string) {
      const state = receiverOf(this, ELEMENT_RULE);
      (state.classList ??= listOf((this as unknown as Element).__node_id)).value = value;
    },
  };
  // The setter takes text and the getter returns the list, which a single property type cannot say.
  return members as unknown as ClassMembers;
}
