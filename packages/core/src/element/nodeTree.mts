// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The node members of every handle: identity, data and links read from the host tree.

import {
  appendChild,
  childNodeIds,
  getAttribute,
  isConnected,
  linkedNode,
  removeChild,
  setAttribute,
  textContent,
} from "../ffi/index.mts";
import { nullableText } from "./arguments.mts";
import { nodeListOf } from "./collections.mts";
import {
  entered,
  isDestroyed,
  left,
  nodeOf,
  receiverOf,
  NODE_RULE,
  receiverNode,
} from "./lifecycle.mts";
import type { NodeLink } from "../ffi/index.mts";
import type { IncaDocument } from "../types.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type { NodeMembers } from "./nodeTypes.mts";
import type { IncaElement, IncaFragment, IncaNode } from "./types.mts";

/** The members {@link nodeTreeMembers} builds. */
export type NodeTreeMembers = { textContent: string | null } & Pick<
  NodeMembers,
  | "nodeType"
  | "nodeName"
  | "nodeValue"
  | "isConnected"
  | "ownerDocument"
  | "parentNode"
  | "firstChild"
  | "lastChild"
  | "previousSibling"
  | "nextSibling"
  | "childNodes"
  | "hasChildNodes"
  | "parentElement"
>;

const NODE_TYPES = { element: 1, text: 3, comment: 8, document: 9, fragment: 11 } as const;

/**
 * Reads the data of a text node or comment. A freed node keeps the last data of a text
 * node and reads `""` for a comment.
 * @param node - the text node or comment
 * @returns the data
 */
export function readData(node: IncaNode): string {
  if (isDestroyed(node)) return node.kind === "text" ? node.text : "";
  return String(getAttribute(node.__node_id, "value") ?? "");
}

/**
 * Stores the data of a text node or comment. An element, the document and a freed node
 * ignore it.
 * @param node - the text node or comment
 * @param data - the new data
 */
export function writeData(node: IncaNode, data: string): void {
  if (node.kind === "element" || node.kind === "fragment" || node.kind === "document") return;
  if (isDestroyed(node)) return;
  setAttribute(node.__node_id, "value", data);
  if (node.kind === "text") node.text = data;
}

/**
 * Replaces all children of `parent` with at most a single text child holding `text`. The old
 * children keep their listeners. An empty string leaves `parent` childless.
 * @param parent - the element or fragment whose children are replaced
 * @param text - the new text content
 * @param builders - the functions that build the handles
 */
export function replaceWithText(parent: IncaNode, text: string, builders: HandleBuilders): void {
  for (const childId of childNodeIds(parent.__node_id)) {
    removeChild(parent.__node_id, childId);
    const child = nodeOf(childId);
    if (child) left(child);
  }
  if (!text) return;

  const textNode = builders.createText!(text);
  appendChild(parent.__node_id, textNode.__node_id);
  entered(textNode);
}

/**
 * Builds the node members of `Node`. Each read asks the host tree again.
 * @param builders - the lookups of the document handle and of a handle by node id
 * @returns the members, as getters
 */
export function nodeTreeMembers(builders: HandleBuilders): NodeTreeMembers {
  const reach = (self: unknown, link: NodeLink, elementOnly = false): IncaNode | null => {
    const node = receiverNode(self, NODE_RULE);
    if (isDestroyed(node)) return null;
    const found = linkedNode(node.__node_id, link, elementOnly);
    return found === null ? null : (builders.node(found) ?? null);
  };
  const liveChildren = (node: IncaNode): IncaNode[] =>
    isDestroyed(node)
      ? []
      : childNodeIds(node.__node_id).flatMap((child) => builders.node(child) ?? []);
  return {
    get nodeType() {
      return NODE_TYPES[receiverNode(this, NODE_RULE).kind];
    },
    get nodeName() {
      const node = receiverNode(this, NODE_RULE);
      if (node.kind === "element") return node.tagName;
      return node.kind === "fragment" ? "#document-fragment" : `#${node.kind}`;
    },
    get nodeValue() {
      const node = receiverNode(this, NODE_RULE);
      return node.kind === "element" || node.kind === "fragment" || node.kind === "document"
        ? null
        : readData(node);
    },
    set nodeValue(value: string | null) {
      writeData(receiverNode(this, NODE_RULE), String(value ?? ""));
    },
    get textContent() {
      const node = receiverNode(this, NODE_RULE);
      switch (node.kind) {
        case "text":
        case "comment":
          return readData(node);
        case "document":
          return null;
        default:
          return isDestroyed(node) ? "" : textContent(node.__node_id);
      }
    },
    set textContent(value: string | null) {
      const node = receiverNode(this, NODE_RULE);
      switch (node.kind) {
        case "text":
        case "comment":
          writeData(node, nullableText(value));
          return;
        case "document":
          return;
        default:
          if (!isDestroyed(node)) replaceWithText(node, nullableText(value), builders);
      }
    },
    get isConnected() {
      const node = receiverNode(this, NODE_RULE);
      return !isDestroyed(node) && isConnected(node.__node_id);
    },
    get ownerDocument() {
      return receiverNode(this, NODE_RULE).kind === "document" ? null : builders.document();
    },
    get parentNode() {
      return reach(this, "parent") as IncaElement | IncaFragment | IncaDocument | null;
    },
    get parentElement() {
      return reach(this, "parent", true) as IncaElement | null;
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
      const node = this as unknown as IncaNode;
      return (state.childNodes ??= nodeListOf(() => liveChildren(node)));
    },
    hasChildNodes() {
      return liveChildren(receiverNode(this, NODE_RULE)).length > 0;
    },
  };
}
