// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The classic mutation members of every handle. The host validates and performs each change, and
// the members keep the node table in step with it.

import { appendChild, insertBefore, removeChild, replaceChild } from "../ffi/index.mts";
import { nodeArgument, requireArguments } from "./arguments.mts";
import { cloneNode } from "./clone.mts";
import { entered, isDestroyed, left } from "./lifecycle.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type { IncaNode, NodeMembers } from "./types.mts";

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
 * Builds the mutation members of `node`.
 * @param node - the handle the members belong to
 * @param builders - the functions that build the handles of a clone
 * @returns the members
 */
export function nodeMutationMembers(node: IncaNode, builders: HandleBuilders): NodeMutationMembers {
  const id = node.__node_id;
  /** Makes the handle of an inserted node strong. The children of a fragment are strong already. */
  const enter = (child: IncaNode): void => {
    if (child.kind !== "fragment") entered(child);
  };
  return {
    appendChild: (...args: [node: IncaNode]) => {
      requireArguments(args, 1, "appendChild");
      const child = nodeArgument(args[0], 1);
      appendChild(id, child.__node_id);
      enter(child);
      return child;
    },
    insertBefore: (...args: [node: IncaNode, child: IncaNode | null | undefined]) => {
      requireArguments(args, 2, "insertBefore");
      const child = nodeArgument(args[0], 1);
      const reference = args[1] === null || args[1] === undefined ? null : nodeArgument(args[1], 2);
      if (reference && isDestroyed(reference)) throw notFound();
      insertBefore(id, child.__node_id, reference?.__node_id ?? null);
      enter(child);
      return child;
    },
    removeChild: (...args: [child: IncaNode]) => {
      requireArguments(args, 1, "removeChild");
      const child = nodeArgument(args[0], 1);
      if (isDestroyed(child)) throw notFound();
      removeChild(id, child.__node_id);
      left(child);
      return child;
    },
    replaceChild: (...args: [node: IncaNode, child: IncaNode]) => {
      requireArguments(args, 2, "replaceChild");
      const child = nodeArgument(args[0], 1);
      const old = nodeArgument(args[1], 2);
      if (isDestroyed(old)) throw notFound();
      replaceChild(id, child.__node_id, old.__node_id);
      enter(child);
      if (old !== child) left(old);
      return old;
    },
    cloneNode: (deep?: boolean) => cloneNode(node, Boolean(deep), builders),
  } as NodeMutationMembers;
}
