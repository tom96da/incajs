// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The classic mutation members of every handle. The host validates and performs each change, and
// the members patch the shadow tree with the position it returns.

import {
  appendChild,
  cloneNode as cloneHostNode,
  insertBefore,
  nameOf,
  removeChild,
  replaceChild,
} from "../tree.mts";
import { requireArguments } from "./attributes.mts";
import { existingContent, isDestroyed, isNode } from "./lifecycle.mts";
import { keep, link, unlink } from "./shadowTree.mts";
import type { ElementName } from "../native.mts";
import type { IncaDocument, NodeId } from "../types.mts";
import type {
  IncaElement,
  IncaFragment,
  IncaNode,
  IncaParent,
  IncaText,
  NodeMembers,
} from "./types.mts";

/** The members {@link nodeMutationMembers} builds. */
export type NodeMutationMembers = Pick<
  NodeMembers,
  "appendChild" | "insertBefore" | "removeChild" | "replaceChild" | "cloneNode"
>;

/**
 * The functions that build and find the handles of native nodes. Every handle passes `text`,
 * `document` and `node`. An element or a fragment also passes `element` and `fragment`, the
 * builders of the kinds its clone can hold.
 */
export interface HandleBuilders {
  element?: (id: NodeId, kind: IncaElement["kind"], name?: ElementName) => IncaElement;
  text: (id: NodeId, data: string) => IncaText;
  fragment?: (id: NodeId) => IncaFragment;
  /** The handle of the document. */
  document: () => IncaDocument;
  /** The handle of a node by id: the live handle, or the document handle for the document id. */
  node: (id: NodeId) => IncaNode | undefined;
}

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
 * @internal
 */
export function nodeArgument(value: unknown, position: number): IncaNode {
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
 * Copies a node into handles. The host copies the nodes and the handles follow its pre-order.
 * @param node - the node to copy
 * @param deep - whether the descendants are copied too
 * @param builders - the functions that build the handles
 * @returns the held, parentless copy
 */
export function cloneNode<T extends IncaNode>(node: T, deep: boolean, builders: HandleBuilders): T {
  const copies = cloneHostNode(node.__node_id, deep);
  let next = 0;
  // The document has no clone and no parent, so no copy walk reaches it.
  type Copyable = Exclude<IncaNode, IncaDocument>;
  const build = (original: Copyable): IncaNode => {
    const entry = copies[next++]!;
    const id = entry[0];
    if (original.kind === "text") return builders.text(id, original.text);
    const copy =
      original.kind === "fragment"
        ? builders.fragment!(id)
        : builders.element!(
            id,
            original.kind,
            original.kind === "element" ? nameOf(entry) : undefined,
          );
    if (original.kind === "element" && existingContent(original) !== undefined) {
      next++;
      const content = existingContent(copy as IncaElement)!;
      if (deep) {
        for (const child of existingContent(original)!.__children)
          link(content, [build(child as Copyable)], content.__children.length);
      }
    }
    if (!deep) return copy;
    for (const child of original.__children)
      link(copy as IncaParent, [build(child as Copyable)], copy.__children.length);
    return copy;
  };
  const copy = build(node as Copyable);
  keep(copy);
  return copy as T;
}

/**
 * Builds the mutation members of `node`.
 * @param node - the handle the members belong to
 * @param builders - the functions that build the handles of a clone
 * @returns the members
 */
export function nodeMutationMembers(node: IncaNode, builders: HandleBuilders): NodeMutationMembers {
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
    cloneNode: (deep?: boolean) => cloneNode(node, Boolean(deep), builders),
  } as NodeMutationMembers;
}
