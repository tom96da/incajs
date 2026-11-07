// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The style elements of the stylesheets that built code applies.

import { createElementNode, createIncaElement, rootElement } from "./element/handle.mts";
import { insert, link, remove, unlink } from "./element/shadowTree.mts";
import { createIncaText, createTextNode } from "./element/text.mts";
import {
  childNodeIds,
  getAttribute,
  linkedNode,
  queryNodes,
  removeChild,
  rootNodeId,
  setAttribute,
} from "./tree.mts";
import type { IncaElement } from "./element/types.mts";
import type { NodeId } from "./types.mts";

const KEY_ATTRIBUTE = "data-inca-style";

const elements = new Map<string, IncaElement>();
let scanned = false;

/** Gives a style element an earlier evaluation left a handle, and links it into the shadow tree. */
function adopt(id: NodeId): IncaElement {
  const el = createIncaElement(id, "element", "style");
  for (const child of childNodeIds(id)) {
    const text = createIncaText(child, String(getAttribute(child, "value") ?? ""));
    link(el, [text], el.__children.length);
  }
  const root = rootElement();
  if (linkedNode(id, "parent") === root.__node_id) {
    const host = childNodeIds(root.__node_id);
    const before = root.__children.filter((c) => host.indexOf(c.__node_id) < host.indexOf(id));
    link(root, [el], before.length);
  }
  return el;
}

/** Finds the elements an earlier evaluation of the app left in the tree. */
function scan(): void {
  if (scanned) return;
  for (const id of queryNodes(rootNodeId(), `style[${KEY_ATTRIBUTE}]`)) {
    elements.set(String(getAttribute(id, KEY_ATTRIBUTE)), adopt(id));
  }
  scanned = true;
}

/** Replaces the children of `el` with one text node holding `css`. */
function writeText(el: IncaElement, css: string): void {
  const old = el.__children.slice();
  if (css) insert(createTextNode(css), el);
  for (const child of old) remove(child);
}

/**
 * Applies a stylesheet. The first call for `key` appends a `style` element to the root container,
 * and later calls replace its text.
 * @param key - the identity of the stylesheet
 * @param css - the text of the stylesheet
 * @internal
 */
export function updateStyle(key: string, css: string): void {
  scan();
  const known = elements.get(key);
  if (known !== undefined) {
    writeText(known, css);
    return;
  }

  const el = createElementNode("style");
  setAttribute(el.__node_id, KEY_ATTRIBUTE, key);
  writeText(el, css);
  insert(el, rootElement());
  elements.set(key, el);
}

/**
 * Removes the `style` element of a stylesheet. An unknown `key` does nothing.
 * @param key - the identity given to {@link updateStyle}
 * @internal
 */
export function removeStyle(key: string): void {
  scan();
  const el = elements.get(key);
  if (el === undefined) return;

  const parent = linkedNode(el.__node_id, "parent");
  if (parent !== null) removeChild(parent, el.__node_id);
  unlink(el);
  elements.delete(key);
}
