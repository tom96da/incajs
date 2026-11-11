// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The markup members of an element. The host parses and serializes, and the members build the
// handles of the parsed nodes and patch the shadow tree.

import {
  insertAdjacentHtml,
  innerHtml,
  nameOfRow,
  outerHtml,
  setInnerHtml,
  setOuterHtml,
} from "../tree.mts";
import { attributeText, requireArguments } from "./attributes.mts";
import { existingContent, isDestroyed } from "./lifecycle.mts";
import { keep, link, unlink } from "./shadowTree.mts";
import type { Parsed } from "../native.mts";
import type { NodeId } from "../types.mts";
import type { HandleBuilders } from "./nodeMutation.mts";
import type { HtmlMembers, IncaElement, IncaNode, IncaParent } from "./types.mts";

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
 * Builds the handles of a parse result and links the nested ones. The entries come in pre-order, so
 * a parent has its handle before its children. The content fragment of a template takes the handle
 * the template already holds.
 * @param parsed - what the host parsed
 * @param builders - the functions that build the handles of each kind
 * @returns the handles whose parent is no entry of the result, in order
 */
function adopt(parsed: Parsed, builders: HandleBuilders): IncaNode[] {
  const handles = new Map<NodeId, IncaNode>();
  const roots: IncaNode[] = [];
  for (const row of parsed[1]) {
    const [id, tag, , parentId, data] = row;
    const parent = handles.get(parentId) as IncaParent | undefined;
    if (tag === "#fragment") {
      handles.set(id, existingContent(parent as IncaElement)!);
      continue;
    }
    const handle =
      tag === "#text"
        ? builders.text(id, data ?? "")
        : tag === "#comment"
          ? builders.element!(id, "comment")
          : builders.element!(id, "element", nameOfRow(row));
    handles.set(id, handle);
    if (parent) link(parent, [handle], parent.__children.length);
    else roots.push(handle);
  }
  return roots;
}

/**
 * Builds the markup members of `element`.
 * @param element - the handle the members belong to
 * @param builders - the functions that build the handles of a parse result
 * @returns the members
 */
export function htmlMembers(element: IncaElement, builders: HandleBuilders): HtmlMembers {
  const id = element.__node_id;
  const place = (parsed: Parsed, container: IncaParent): void =>
    link(container, adopt(parsed, builders), parsed[0]);
  return {
    get innerHTML() {
      return isDestroyed(element) ? "" : innerHtml(id);
    },
    set innerHTML(value: string) {
      const text = markupText(value);
      if (isDestroyed(element)) return;
      const container = existingContent(element) ?? element;
      const parsed = setInnerHtml(id, text);
      for (const child of container.__children.slice()) {
        unlink(child);
        keep(child);
      }
      place(parsed, container);
    },
    get outerHTML() {
      return isDestroyed(element) ? "" : outerHtml(id);
    },
    set outerHTML(value: string) {
      const text = markupText(value);
      if (isDestroyed(element)) return;
      const parsed = setOuterHtml(id, text);
      const parent = element.parent!;
      if (parsed === null) return;
      unlink(element);
      keep(element);
      place(parsed, parent);
    },
    insertAdjacentHTML: (...args: [position: string, text: string]) => {
      requireArguments(args, 2, "insertAdjacentHTML");
      const position = attributeText(args[0]);
      const text = attributeText(args[1]);
      if (isDestroyed(element)) return;
      const parsed = insertAdjacentHtml(id, position, text);
      const beside = /^(beforebegin|afterend)$/i.test(position);
      place(parsed, beside ? element.parent! : element);
    },
  };
}
