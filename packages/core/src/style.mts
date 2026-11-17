// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The style elements of the stylesheets that built code applies.

import {
  adoptTextNode,
  createElementNode,
  createIncaElement,
  createTextNode,
  rootElement,
} from "./element/handle.mts";
import { entered, nodeOf } from "./element/lifecycle.mts";
import { insert, remove } from "./element/treeOps.mts";
import { childNodeIds, getAttribute, queryNodes, rootNodeId, setAttribute } from "./ffi/index.mts";
import type { Element } from "./element/types.mts";
import type { NodeId } from "./types.mts";

const KEY_ATTRIBUTE = "data-inca-style";

const elements = new Map<string, Element>();
let scanned = false;

/** Gives a style element an earlier evaluation left a handle, and makes it strong with its text. */
function adopt(id: NodeId): Element {
  const el = createIncaElement(id, "style");
  for (const child of childNodeIds(id)) {
    entered(adoptTextNode(child, String(getAttribute(child, "value") ?? "")));
  }
  entered(el);
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
function writeText(el: Element, css: string): void {
  const old = childNodeIds(el.__node_id);
  if (css) insert(createTextNode(css), el);
  for (const child of old) {
    const text = nodeOf(child);
    if (text) remove(text);
  }
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

  remove(el);
  elements.delete(key);
}
