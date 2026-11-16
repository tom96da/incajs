// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The classic mutation members of every handle. The host validates and performs each change, and
// the members keep the node table in step with it.

import { appendChild, insertBefore, removeChild, replaceChild } from "../ffi/index.mts";
import { nodeArgument, requireArguments } from "./arguments.mts";
import { cloneNode } from "./clone.mts";
import { entered, isDestroyed, left, NODE_RULE, receiverNode } from "./lifecycle.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type { NodeMembers } from "./nodeTypes.mts";
import type { IncaNode } from "./types.mts";

/** The members {@link nodeMutationMembers} builds. */
export type NodeMutationMembers = Pick<
  NodeMembers,
  "appendChild" | "insertBefore" | "removeChild" | "replaceChild" | "cloneNode"
>;

/**
 * Builds the error for a child the host freed.
 * @returns a `NotFoundError`
 */
export function notFound(): Error {
  return new DOMException(
    "the node was freed. pass a child of the node you call it on",
    "NotFoundError",
  );
}

/**
 * Builds the mutation members of `Node`.
 * @param builders - the functions that build the handles of a clone
 * @returns the members
 */
export function nodeMutationMembers(builders: HandleBuilders): NodeMutationMembers {
  const nodeOf = (self: unknown): IncaNode => receiverNode(self, NODE_RULE);
  /** Makes the handle of an inserted node strong. The children of a fragment are strong already. */
  const enter = (child: IncaNode): void => {
    if (child.kind !== "fragment") entered(child);
  };
  return {
    appendChild(node: IncaNode) {
      const self = nodeOf(this);
      requireArguments(arguments, 1, "appendChild");
      const child = nodeArgument(node, 1);
      appendChild(self.__node_id, child.__node_id);
      enter(child);
      return child;
    },
    insertBefore(node: IncaNode, child: IncaNode | null | undefined) {
      const self = nodeOf(this);
      requireArguments(arguments, 2, "insertBefore");
      const inserted = nodeArgument(node, 1);
      const reference = child === null || child === undefined ? null : nodeArgument(child, 2);
      if (reference && isDestroyed(reference)) throw notFound();
      insertBefore(self.__node_id, inserted.__node_id, reference?.__node_id ?? null);
      enter(inserted);
      return inserted;
    },
    removeChild(child: IncaNode) {
      const self = nodeOf(this);
      requireArguments(arguments, 1, "removeChild");
      const removed = nodeArgument(child, 1);
      if (isDestroyed(removed)) throw notFound();
      removeChild(self.__node_id, removed.__node_id);
      left(removed);
      return removed;
    },
    replaceChild(node: IncaNode, child: IncaNode) {
      const self = nodeOf(this);
      requireArguments(arguments, 2, "replaceChild");
      const inserted = nodeArgument(node, 1);
      const old = nodeArgument(child, 2);
      if (isDestroyed(old)) throw notFound();
      replaceChild(self.__node_id, inserted.__node_id, old.__node_id);
      enter(inserted);
      if (old !== inserted) left(old);
      return old;
    },
    cloneNode(...rest: [deep?: boolean]) {
      return cloneNode(nodeOf(this), Boolean(rest[0]), builders);
    },
  } as NodeMutationMembers;
}
