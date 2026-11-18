// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The markup members of an element. The host parses and serializes, and the members keep the node
// table in step.

import { childNodeIds, insertAdjacentHtml, setInnerHtml, setOuterHtml } from "../ffi/index.mts";
import { attributeText, requireArguments } from "./arguments.mts";
import {
  existingContent,
  isDestroyed,
  left,
  nodeOf,
  ELEMENT_RULE,
  receiverNode,
} from "./lifecycle.mts";
import type { HtmlMembers, Element } from "./types.mts";

/**
 * Converts an assignment to `innerHTML` or `outerHTML`: `null` gives `""` and any other value its
 * text. A `Symbol` throws a `TypeError`.
 * @param value - the assigned value
 * @returns the markup
 */
function markupText(value: unknown): string {
  return value === null ? "" : attributeText(value);
}

/**
 * Builds the markup members of `Element`. The host defines the getters of `innerHTML` and
 * `outerHTML`, and `defineMembers` keeps them beside these setters.
 * @returns the members
 */
export function htmlMembers(): HtmlMembers {
  const elementOf = (self: unknown): Element => receiverNode<Element>(self, ELEMENT_RULE);
  return {
    set innerHTML(value: string) {
      const element = elementOf(this);
      const text = markupText(value);
      if (isDestroyed(element)) return;
      const container = existingContent(element) ?? element;
      const old = childNodeIds(container.__node_id);
      setInnerHtml(element.__node_id, text);
      for (const child of old) {
        const handle = nodeOf(child);
        if (handle) left(handle);
      }
    },
    set outerHTML(value: string) {
      const element = elementOf(this);
      const text = markupText(value);
      if (isDestroyed(element)) return;
      if (setOuterHtml(element.__node_id, text)) left(element);
    },
    insertAdjacentHTML(position: string, text: string) {
      const element = elementOf(this);
      requireArguments(arguments, 2, "insertAdjacentHTML");
      const where = attributeText(position);
      const markup = attributeText(text);
      if (isDestroyed(element)) return;
      insertAdjacentHtml(element.__node_id, where, markup);
    },
  };
}
