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
      if (node.kind === "element") return null;
      if (isDestroyed(node)) return node.kind === "text" ? node.text : "";
      return String(getAttribute(id, "value") ?? "");
    },
    set nodeValue(value: string | null) {
      if (node.kind === "element" || isDestroyed(node)) return;
      const data = String(value ?? "");
      setAttribute(id, "value", data);
      if (node.kind === "text") node.text = data;
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
