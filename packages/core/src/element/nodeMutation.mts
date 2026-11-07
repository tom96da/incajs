// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The classic mutation members of every handle. The host validates and performs each change, and
// the members patch the shadow tree with the position it returns.

import { appendChild, insertBefore, removeChild, replaceChild } from "../tree.mts";
import { requireArguments } from "./attributes.mts";
import { isDestroyed, isNode } from "./lifecycle.mts";
import { keep, link, unlink } from "./shadowTree.mts";
import type { IncaNode, IncaParent, NodeMembers } from "./types.mts";

/** The members {@link nodeMutationMembers} builds. */
export type NodeMutationMembers = Pick<
  NodeMembers,
  "appendChild" | "insertBefore" | "removeChild" | "replaceChild"
>;

/**
 * Builds the error for a child the host freed.
 * @returns a `NotFoundError`
 */
function notFound(): Error {
  return new DOMException(
    "the node was freed. pass a child of the node you call it on",
    "NotFoundError",
  );
}

/**
 * Checks that an argument is a node.
 * @param value - the argument
 * @param position - the argument's place in the call, counted from 1
 * @returns the node
 * @throws {TypeError} when `value` is not a node
 */
function nodeArgument(value: unknown, position: number): IncaNode {
  if (!isNode(value)) {
    throw new TypeError(
      `parameter ${position} is not a node. pass an element, a text node, a comment or a fragment`,
    );
  }
  return value;
}

/**
 * Lists the nodes an insertion moves: the node, or the children of a fragment.
 * @param node - the inserted node
 * @returns the moved nodes in order
 */
function movedBy(node: IncaNode): IncaNode[] {
  return node.kind === "fragment" ? node.__children.slice() : [node];
}

/**
 * Builds the mutation members of `node`.
 * @param node - the handle the members belong to
 * @returns the members
 */
export function nodeMutationMembers(node: IncaNode): NodeMutationMembers {
  const id = node.__node_id;
  // A text node as the parent throws in the host call, so the shadow patch sees an element or a
  // fragment.
  const parent = node as IncaParent;
  const insert = (child: IncaNode, call: () => number): void => {
    const moved = movedBy(child);
    const position = call();
    for (const mover of moved) unlink(mover);
    link(parent, moved, position);
  };
  return {
    appendChild: (...args: [node: IncaNode]) => {
      requireArguments(args, 1, "appendChild");
      const child = nodeArgument(args[0], 1);
      insert(child, () => appendChild(id, child.__node_id));
      return child;
    },
    insertBefore: (...args: [node: IncaNode, child: IncaNode | null | undefined]) => {
      requireArguments(args, 2, "insertBefore");
      const child = nodeArgument(args[0], 1);
      const reference = args[1] === null || args[1] === undefined ? null : nodeArgument(args[1], 2);
      if (reference && isDestroyed(reference)) throw notFound();
      insert(child, () => insertBefore(id, child.__node_id, reference?.__node_id ?? null));
      return child;
    },
    removeChild: (...args: [child: IncaNode]) => {
      requireArguments(args, 1, "removeChild");
      const child = nodeArgument(args[0], 1);
      if (isDestroyed(child)) throw notFound();
      removeChild(id, child.__node_id);
      unlink(child);
      keep(child);
      return child;
    },
    replaceChild: (...args: [node: IncaNode, child: IncaNode]) => {
      requireArguments(args, 2, "replaceChild");
      const child = nodeArgument(args[0], 1);
      const old = nodeArgument(args[1], 2);
      if (isDestroyed(old)) throw notFound();
      const moved = movedBy(child);
      const position = replaceChild(id, child.__node_id, old.__node_id);
      for (const mover of moved) unlink(mover);
      unlink(old);
      if (old !== child) keep(old);
      link(parent, moved, position);
      return old;
    },
  } as NodeMutationMembers;
}
