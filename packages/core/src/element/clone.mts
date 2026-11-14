// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The copy of a node: the host copies the nodes and builds one handle for each entry it returns.

import { cloneNode as cloneHostNode, nameOf } from "../ffi/index.mts";
import { entered, hold } from "./lifecycle.mts";
import type { ElementEntry, NodeEntry } from "../ffi/index.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type { IncaNode } from "./types.mts";

/**
 * Builds the handle of one copied node.
 * @param entry - what the host returned for the copy
 * @param index - the place of the entry in the result
 * @param builders - the functions that build the handles
 * @returns the handle, or `undefined` for the content fragment of a template, which the handle of
 *   the template already holds
 */
function buildCopy(
  entry: NodeEntry | ElementEntry,
  index: number,
  builders: HandleBuilders,
): IncaNode | undefined {
  if (entry.length === 5) return builders.element!(entry[0], "element", nameOf(entry));
  switch (entry[1]) {
    case "#text":
      return builders.text(entry[0], entry[2] ?? "");
    case "#comment":
      return builders.element!(entry[0], "comment");
    case "#document":
      return builders.documentCopy(entry[0]);
    default:
      return index === 0 ? builders.fragment!(entry[0]) : undefined;
  }
}

/**
 * Copies a node into handles. The host copies the nodes and the handles follow its pre-order.
 * @param node - the node to copy
 * @param deep - whether the descendants are copied too
 * @param builders - the functions that build the handles
 * @returns the held, parentless copy
 */
export function cloneNode<T extends IncaNode>(node: T, deep: boolean, builders: HandleBuilders): T {
  const entries = cloneHostNode(node.__node_id, deep);
  const copy = buildCopy(entries[0]!, 0, builders)!;
  for (const [index, entry] of entries.entries()) {
    if (index === 0) continue;
    const handle = buildCopy(entry, index, builders);
    if (handle) entered(handle);
  }
  hold(copy);
  return copy as T;
}
