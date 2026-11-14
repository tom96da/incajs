// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The markup members of an element. The host parses and serializes, and the members build the
// handles of the parsed nodes and keep the node table in step.

import {
  childNodeIds,
  insertAdjacentHtml,
  innerHtml,
  nameOfRow,
  outerHtml,
  setInnerHtml,
  setOuterHtml,
} from "../ffi/index.mts";
import { attributeText, requireArguments } from "./arguments.mts";
import { entered, existingContent, isDestroyed, left, nodeOf } from "./lifecycle.mts";
import type { Parsed } from "../ffi/index.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type { HtmlMembers, IncaElement } from "./types.mts";

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
 * Builds the handles of a parse result and makes them strong, as the host put each one under a
 * parent. The entries come in pre-order, so a parent has its handle before its children. The
 * content fragment of a template takes the handle the template already holds.
 * @param parsed - what the host parsed
 * @param builders - the functions that build the handles of each kind
 */
function adopt(parsed: Parsed, builders: HandleBuilders): void {
  for (const row of parsed[1]) {
    const [id, tag] = row;
    if (tag === "#fragment") continue;
    const handle =
      tag === "#text"
        ? builders.text(id, row[4] ?? "")
        : tag === "#comment"
          ? builders.element!(id, "comment")
          : builders.element!(id, "element", nameOfRow(row));
    entered(handle);
  }
}

/**
 * Builds the markup members of `element`.
 * @param element - the handle the members belong to
 * @param builders - the functions that build the handles of a parse result
 * @returns the members
 */
export function htmlMembers(element: IncaElement, builders: HandleBuilders): HtmlMembers {
  const id = element.__node_id;
  return {
    get innerHTML() {
      return isDestroyed(element) ? "" : innerHtml(id);
    },
    set innerHTML(value: string) {
      const text = markupText(value);
      if (isDestroyed(element)) return;
      const container = existingContent(element) ?? element;
      const old = childNodeIds(container.__node_id);
      const parsed = setInnerHtml(id, text);
      for (const child of old) {
        const handle = nodeOf(child);
        if (handle) left(handle);
      }
      adopt(parsed, builders);
    },
    get outerHTML() {
      return isDestroyed(element) ? "" : outerHtml(id);
    },
    set outerHTML(value: string) {
      const text = markupText(value);
      if (isDestroyed(element)) return;
      const parsed = setOuterHtml(id, text);
      if (parsed === null) return;
      left(element);
      adopt(parsed, builders);
    },
    insertAdjacentHTML: (...args: [position: string, text: string]) => {
      requireArguments(args, 2, "insertAdjacentHTML");
      const position = attributeText(args[0]);
      const text = attributeText(args[1]);
      if (isDestroyed(element)) return;
      adopt(insertAdjacentHtml(id, position, text), builders);
    },
  };
}
