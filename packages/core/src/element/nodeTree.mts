// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The node members of every handle: identity, data and links read from the host tree.

import {
  appendChild,
  childNodeIds,
  createNode,
  linkedNode,
  removeChild,
  setCharacterData,
} from "../ffi/index.mts";
import { nullableText } from "./arguments.mts";
import { nodeListOf } from "./collections.mts";
import { isDestroyed, left, nodeOf, receiverOf, NODE_RULE, receiverNode } from "./lifecycle.mts";
import type { NodeLink } from "../ffi/index.mts";
import type { Document } from "../types.mts";
import type { HandleNode } from "./handleNode.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type { Node } from "./nodeTypes.mts";
import type { Element, DocumentFragment } from "./types.mts";

/** The members {@link nodeTreeMembers} builds. */
export type NodeTreeMembers = { textContent: string | null } & Pick<
  Node,
  | "ownerDocument"
  | "parentNode"
  | "firstChild"
  | "lastChild"
  | "previousSibling"
  | "nextSibling"
  | "childNodes"
  | "parentElement"
>;

/**
 * Replaces all children of `parent` with at most a single text child holding `text`. The old
 * children keep their listeners. An empty string leaves `parent` childless. A text that cannot
 * become a node leaves the children in place.
 * @param parent - the element or fragment whose children are replaced
 * @param text - the new text content
 * @param builders - the functions that build the handles
 */
export function replaceWithText(parent: Node, text: string, builders: HandleBuilders): void {
  const textId = text ? createNode("#text") : undefined;
  if (textId !== undefined) setCharacterData(textId, text);
  for (const childId of childNodeIds(parent.__node_id)) {
    removeChild(parent.__node_id, childId);
    const child = nodeOf(childId);
    if (child) left(child);
  }
  if (textId === undefined) return;

  appendChild(parent.__node_id, textId);
  builders.node(textId);
}

/**
 * Builds the members of `Node` that return a handle, and the setter of `textContent`. The host
 * defines the other members and the getter of `textContent`.
 * @param builders - the lookups of the document handle and of a handle by node id
 * @returns the members, as getters
 */
export function nodeTreeMembers(builders: HandleBuilders): NodeTreeMembers {
  const reach = (self: unknown, link: NodeLink, elementOnly = false): HandleNode | null => {
    const node = receiverNode(self, NODE_RULE);
    if (isDestroyed(node)) return null;
    const found = linkedNode(node.__node_id, link, elementOnly);
    return found === null ? null : (builders.node(found) ?? null);
  };
  const liveChildren = (node: HandleNode): HandleNode[] =>
    isDestroyed(node) ? [] : childNodeIds(node.__node_id).map((child) => builders.node(child)!);
  return {
    set textContent(value: string | null) {
      const node = receiverNode(this, NODE_RULE);
      switch (node.kind) {
        case "text":
        case "comment":
          if (!isDestroyed(node)) setCharacterData(node.__node_id, nullableText(value));
          return;
        case "document":
          return;
        default:
          if (!isDestroyed(node)) replaceWithText(node, nullableText(value), builders);
      }
    },
    get ownerDocument() {
      return receiverNode(this, NODE_RULE).kind === "document" ? null : builders.document();
    },
    get parentNode() {
      return reach(this, "parent") as Element | DocumentFragment | Document | null;
    },
    get parentElement() {
      return reach(this, "parent", true) as Element | null;
    },
    get firstChild() {
      return reach(this, "first");
    },
    get lastChild() {
      return reach(this, "last");
    },
    get previousSibling() {
      return reach(this, "previous");
    },
    get nextSibling() {
      return reach(this, "next");
    },
    get childNodes() {
      const state = receiverOf(this, NODE_RULE);
      const node = this as unknown as HandleNode;
      return (state.childNodes ??= nodeListOf(() => liveChildren(node)));
    },
  };
}
