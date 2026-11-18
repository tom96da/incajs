// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The copy of a node: the host copies the nodes and core builds the handle of the copy.

import { cloneNode as cloneHostNode } from "../ffi/index.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type { Node } from "./nodeTypes.mts";

/**
 * Copies a node. The host copies the nodes and the handle of the copy is built now. The handles
 * of the copies below it are built when a script reaches them.
 * @param node - the node to copy
 * @param deep - whether the descendants are copied too
 * @param builders - the lookup that builds the handle of the copy
 * @returns the held, parentless copy
 */
export function cloneNode<T extends Node>(node: T, deep: boolean, builders: HandleBuilders): T {
  return builders.node(cloneHostNode(node.__node_id, deep)) as unknown as T;
}
