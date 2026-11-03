// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The node members of every handle: identity, data and links read from the host tree.

import { document } from "../document.mts";
import {
  childNodeIds,
  getAttribute,
  isConnected,
  linkedNode,
  rootNodeId,
  setAttribute,
} from "../tree.mts";
import { nodeListOf } from "./collections.mts";
import { isDestroyed, nodeOf } from "./lifecycle.mts";
import type { NodeLink } from "../native.mts";
import type { IncaElement, IncaNode, NodeMembers } from "./types.mts";

const NODE_TYPES = { element: 1, text: 3, comment: 8 } as const;

/**
 * Reads the data of a text node or comment stand-in. A freed node keeps the last data of a text
 * node and reads `""` for a comment.
 * @param node - the text node or comment stand-in
 * @returns the data
 */
export function readData(node: IncaNode): string {
  if (isDestroyed(node)) return node.kind === "text" ? node.text : "";
  return String(getAttribute(node.__node_id, "value") ?? "");
}

/**
 * Stores the data of a text node or comment stand-in. An element and a freed node ignore it.
 * @param node - the text node or comment stand-in
 * @param data - the new data
 */
export function writeData(node: IncaNode, data: string): void {
  if (node.kind === "element" || isDestroyed(node)) return;
  setAttribute(node.__node_id, "value", data);
  if (node.kind === "text") node.text = data;
}

/**
 * Builds the node members of `node`. Each read asks the host tree again.
 * @param node - the handle the members belong to
 * @returns the members, as getters
 */
export function nodeTreeMembers(node: IncaNode): NodeMembers {
  const id = node.__node_id;
  const reach = (link: NodeLink): IncaNode | null => {
    if (isDestroyed(node)) return null;
    const found = linkedNode(id, link);
    return found === null ? null : (nodeOf(found) ?? null);
  };
  let childNodes: NodeMembers["childNodes"] | undefined;
  const liveChildren = (): IncaNode[] =>
    isDestroyed(node) ? [] : childNodeIds(id).flatMap((child) => nodeOf(child) ?? []);
  return {
    get nodeType() {
      return NODE_TYPES[node.kind];
    },
    get nodeName() {
      return node.kind === "element" ? node.tagName : `#${node.kind}`;
    },
    get nodeValue() {
      return node.kind === "element" ? null : readData(node);
    },
    set nodeValue(value: string | null) {
      writeData(node, String(value ?? ""));
    },
    get isConnected() {
      return !isDestroyed(node) && isConnected(id);
    },
    get ownerDocument() {
      return document;
    },
    get parentNode() {
      const parent = reach("parent");
      if (parent !== null) return parent as IncaElement;
      return id === rootNodeId() ? document : null;
    },
    get firstChild() {
      return reach("first");
    },
    get lastChild() {
      return reach("last");
    },
    get previousSibling() {
      return reach("previous");
    },
    get nextSibling() {
      return reach("next");
    },
    get childNodes() {
      return (childNodes ??= nodeListOf(liveChildren));
    },
    hasChildNodes: () => liveChildren().length > 0,
  };
}
