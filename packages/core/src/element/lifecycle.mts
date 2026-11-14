// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { holdNode, releaseNode } from "../ffi/index.mts";
import { createIdTable } from "../idTable.mts";
import type { ElementName } from "../ffi/index.mts";
import type { IncaDocument, NodeId } from "../types.mts";
import type { IncaElement, IncaFragment, IncaNode, IncaText } from "./types.mts";

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
  /** The handle of a copy of the document, which has the node id of the copy. */
  documentCopy: (id: NodeId) => IncaDocument;
  /**
   * The handle of a node by id: the live handle, or the document handle for the document id. It
   * rebuilds the handle of the root container when that handle was collected.
   */
  node: (id: NodeId) => IncaNode | undefined;
}

/** Every handle that `withNodeId` gave a node id. */
const nodes = new WeakSet<object>();

/**
 * Gives `handle` a read-only, non-enumerable `__node_id`.
 * @param handle - the new handle
 * @param id - the native node's id
 * @returns `handle`
 */
export function withNodeId<T extends object>(
  handle: T,
  id: NodeId,
): T & { readonly __node_id: NodeId } {
  nodes.add(handle);
  return Object.defineProperty(handle, "__node_id", { value: id }) as T & {
    readonly __node_id: NodeId;
  };
}

/**
 * Whether `value` is a node handle.
 * @param value - the value to check
 * @returns `true` for an element, a comment stand-in, a text handle or a fragment
 */
export function isNode(value: unknown): value is IncaNode {
  return typeof value === "object" && value !== null && nodes.has(value);
}

/**
 * The handle of each node that entered a parent and was not removed since. The entry keeps the
 * handle alive.
 */
const strong = createIdTable<IncaNode>();

/**
 * A weak reference to the handle of every other node: a node with no parent, a node below a
 * removed node, and the root container.
 */
const weak = createIdTable<WeakRef<IncaNode>>();

/** The id of the root container, which stays weak while it sits under the document. */
let rootId: NodeId | undefined;

/**
 * Records `node` under its id, weakly. A handle already recorded under the id is replaced.
 * @param node - the handle to record
 */
export function registerNode(node: IncaNode): void {
  strong.delete(node.__node_id);
  weak.set(node.__node_id, new WeakRef(node));
}

/**
 * Records the handle of the root container. The container keeps its weak entry when it enters
 * the tree, so the handle of an unmounted app is collectable.
 * @param node - the root container's handle
 */
export function registerRoot(node: IncaElement): void {
  rootId = node.__node_id;
  registerNode(node);
}

/**
 * Makes the handle of `node` strong, so it lives while the node has a parent. The handles below a
 * removed node keep their weak entries when the node enters a parent again: the node's handle
 * keeps them alive. The root container stays weak.
 * @param node - the handle of a node the host put under a parent
 */
export function entered(node: IncaNode): void {
  const id = node.__node_id;
  if (id === rootId) return;
  strong.set(id, node);
  weak.delete(id);
}

/**
 * Makes the handle of a node that has no parent weak, and holds the node until the handle is
 * collected. The entries below the node do not change.
 * @param node - the handle of a node the host took out of its parent
 */
export function left(node: IncaNode): void {
  unparent(node);
  hold(node);
}

/**
 * The handles `leftHeld` recorded below each removed node at its last removal, which live as long as
 * the handle of that node. A handle moved out of the subtree later stays in the list until the top
 * is collected. The host frees a removed subtree at a frame after the handle of its top is
 * collected.
 */
const kept = new WeakMap<IncaNode, IncaNode[]>();

/**
 * Makes the handle of a node that has no parent weak, for a node the host holds already. Every
 * live handle below it becomes weak too and lives as long as `node`'s handle, whether the entry
 * was strong or weak before. The release of the node registers on the collection of the handle.
 * @param node - the handle of a node that `removeNode` took out of its parent
 * @param nodeIds - the ids of `node` and every node below it
 */
export function leftHeld(node: IncaNode, nodeIds: readonly NodeId[]): void {
  unparent(node);
  const below: IncaNode[] = [];
  for (const id of nodeIds) {
    const handle = nodeOf(id);
    if (handle === undefined || handle === node) continue;
    unparent(handle);
    below.push(handle);
  }
  kept.set(node, below);
  if (!isHeld(node)) releaseWhenCollected(node);
}

/**
 * Moves the entry of `node` to the weak table.
 * @param node - the handle of a node that has no parent, or sits below a removed node
 */
function unparent(node: IncaNode): void {
  const id = node.__node_id;
  weak.set(id, new WeakRef(node));
  strong.delete(id);
}

/**
 * Holds `node` until its handle is collected, so the host does not free it at the next frame. A
 * node held already changes nothing.
 * @param node - the handle of a node that has no parent
 */
export function hold(node: IncaNode): void {
  if (isHeld(node)) return;
  holdNode(node.__node_id);
  releaseWhenCollected(node);
}

/**
 * Looks up the live handle of a native node of any kind.
 * @param id - the native node's id
 * @returns the handle, or `undefined` when none is live
 */
export function nodeOf(id: NodeId): IncaNode | undefined {
  return strong.get(id) ?? weak.get(id)?.deref();
}

/**
 * Looks up the live handle of a native element.
 * @param id - the native node's id
 * @returns the handle, or `undefined` when none is live or the node is not an element
 */
export function elementOf(id: NodeId): IncaElement | undefined {
  const node = nodeOf(id);
  return node?.kind === "element" ? node : undefined;
}

/** The content handle of each template handle. */
const contents = new WeakMap<IncaElement, IncaFragment>();

/**
 * The template handle of each content handle. The entry keeps the template reachable while its
 * content is, so the host holds both nodes as long as either handle is.
 */
const owners = new WeakMap<IncaFragment, IncaElement>();

/**
 * Reads the content handle of a template that already has one.
 * @param node - any node handle
 * @returns the stored content, or `undefined`
 */
export function existingContent(node: IncaNode): IncaFragment | undefined {
  return node.kind === "element" ? contents.get(node) : undefined;
}

/**
 * Links a template handle and its content handle both ways.
 * @param template - the template handle
 * @param content - its content handle
 */
export function storeContent(template: IncaElement, content: IncaFragment): void {
  contents.set(template, content);
  owners.set(content, template);
}

const removed = new WeakSet<IncaNode>();

/**
 * Whether `node` left the tree for good, removed or freed. A removed node stays in the host and
 * its members read real values. Its listeners are gone.
 * @param node - the handle to check
 * @returns `true` after {@link markRemoved} reached `node`, or when the host freed it
 */
export function isRemoved(node: IncaNode): boolean {
  return removed.has(node) || isDestroyed(node);
}

/**
 * Records that the nodes of a removed subtree left the tree. An id with no live handle is
 * skipped.
 * @param nodeIds - the ids of the removed node and every node below it
 */
export function markRemoved(nodeIds: readonly NodeId[]): void {
  for (const id of nodeIds) {
    const node = nodeOf(id);
    if (node) removed.add(node);
  }
}

/**
 * Whether the host freed `node` with its subtree.
 * @param node - the handle to check
 * @returns `true` when no table holds `node` under its id
 */
export function isDestroyed(node: IncaNode): boolean {
  return nodeOf(node.__node_id) !== node;
}

/**
 * Drops the table entries of the nodes the host freed. The ids already list every node of a freed
 * subtree.
 * @param nodeIds - ids of nodes the host freed, subtrees included
 * @internal
 */
export function markFreed(nodeIds: readonly NodeId[]): void {
  for (const id of nodeIds) {
    strong.delete(id);
    weak.delete(id);
  }
}

/**
 * Releases the hold of a node whose handle was collected. A node destroyed
 * already is unknown to the host and the call does nothing.
 */
const released = new FinalizationRegistry<NodeId>((id) => releaseNode(id));

const held = new WeakSet<IncaNode>();

/**
 * Whether {@link releaseWhenCollected} already covers `handle`.
 * @param handle - the handle to check
 * @returns `true` after `releaseWhenCollected(handle)`
 * @internal
 */
export function isHeld(handle: IncaNode): boolean {
  return held.has(handle);
}

/**
 * Releases the hold of `handle`'s node once the handle is collected.
 * @param handle - the handle of a node held with `holdNode`
 * @internal
 */
export function releaseWhenCollected(handle: IncaNode): void {
  held.add(handle);
  released.register(handle, handle.__node_id);
}
