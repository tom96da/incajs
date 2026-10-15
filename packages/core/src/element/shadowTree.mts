// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { createIncaElement } from "./handle.mts";
import { markDestroyed } from "./lifecycle.mts";
import type { IncaCore, TagName } from "../rendererCore.mts";
import type { IncaElement, IncaNode, IncaText } from "./types.mts";

/**
 * Allocates a new element node for `tag`.
 * @param core - the incajs bindings to drive the native tree through
 * @param tag - the node's element kind
 * @returns the new, parentless, childless element
 */
export function createElementNode(core: IncaCore, tag: TagName): IncaElement {
  const id = core.createNode(tag);
  return createIncaElement(core, id, "element", tag);
}

/**
 * Creates a text leaf.
 * @param core - the incajs bindings to drive the native tree through
 * @param text - the initial text content
 * @returns the new text leaf
 */
export function createTextNode(core: IncaCore, text: string): IncaText {
  const id = core.createNode("text");
  core.setAttribute(id, "value", text);
  return { id, kind: "text", parent: null, text };
}

// inca has no comment concept. A hidden, childless container stands in for one.
/**
 * Creates the hidden container that stands in for a comment node, such as a
 * placeholder for conditional content.
 * @param core - the incajs bindings to drive the native tree through
 * @returns the new hidden element
 */
export function createCommentNode(core: IncaCore): IncaElement {
  const id = core.createNode("div");
  core.setStyle(id, "display", "none");
  return createIncaElement(core, id, "comment");
}

/**
 * Updates a text leaf's content in place.
 * @param core - the incajs bindings to drive the native tree through
 * @param node - the text leaf to update
 * @param text - the new text content
 */
export function setText(core: IncaCore, node: IncaText, text: string): void {
  core.setAttribute(node.id, "value", text);
  node.text = text;
}

/**
 * Replaces all of `el`'s children with at most a single text child holding
 * `text`. An empty string leaves `el` childless.
 * @param core - the incajs bindings to drive the native tree through
 * @param el - the container whose children are replaced
 * @param text - the new text content
 */
export function setElementText(core: IncaCore, el: IncaElement, text: string): void {
  for (const child of el.children.splice(0)) {
    core.destroyNode(child.id);
    markDestroyed(child);
    child.parent = null;
  }
  if (!text) return;

  const textNode = createTextNode(core, text);
  core.appendChild(el.id, textNode.id);
  textNode.parent = el;
  el.children.push(textNode);
}

// Drops `child` from its parent in this module's shadow of the native tree.
function unlink(child: IncaNode): void {
  const parent = child.parent;
  if (!parent) return;

  const index = parent.children.indexOf(child);
  if (index !== -1) parent.children.splice(index, 1);
  child.parent = null;
}

// `destroyNode` detaches on its own, so the removal path calls `unlink` alone.
function detach(core: IncaCore, child: IncaNode): void {
  if (child.parent) core.removeChild(child.parent.id, child.id);
  unlink(child);
}

/**
 * Attaches `child` to `parent`, before `anchor` or at the end when `anchor` is
 * omitted or `null`. If `anchor` is `child` itself and `child` is already in
 * `parent`, nothing changes. A `child` attached elsewhere is detached first,
 * so a move keeps the node alive.
 * @param core - the incajs bindings to drive the native tree through
 * @param child - the node being attached
 * @param parent - the new container
 * @param anchor - the sibling to insert before, or omitted/`null` to append at the end
 */
export function insert(
  core: IncaCore,
  child: IncaNode,
  parent: IncaElement,
  anchor?: IncaNode | null,
): void {
  // A child that is its own anchor stays where it is.
  if (anchor === child && child.parent === parent) return;
  detach(core, child);

  const anchorIndex = anchor ? parent.children.indexOf(anchor) : -1;
  core.insertBefore(parent.id, child.id, anchor && anchorIndex !== -1 ? anchor.id : null);
  if (anchorIndex === -1) {
    parent.children.push(child);
  } else {
    parent.children.splice(anchorIndex, 0, child);
  }
  child.parent = parent;
}

/**
 * Removes `child` for good: the native node, its whole subtree, and every
 * event listener registered in it.
 *
 * A caller that removes a subtree calls this on its root alone, and the host
 * frees the descendants with it.
 * @param core - the incajs bindings to drive the native tree through
 * @param child - the node being removed
 */
export function remove(core: IncaCore, child: IncaNode): void {
  unlink(child);
  core.destroyNode(child.id);
  markDestroyed(child);
}

/**
 * Looks up the current parent.
 * @param node - the node to query
 * @returns the current parent, or `null` if it isn't attached
 */
export function parentNode(node: IncaNode): IncaElement | null {
  return node.parent;
}

/**
 * Looks up the next sibling.
 * @param node - the node to query
 * @returns the sibling immediately after `node` in its parent's children, or `null` if there is none (or `node` isn't attached)
 */
export function nextSibling(node: IncaNode): IncaNode | null {
  const parent = node.parent;
  if (!parent) return null;

  const index = parent.children.indexOf(node);
  return parent.children[index + 1] ?? null;
}
