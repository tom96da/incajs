// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The node members of every handle: identity, data and links read from the host tree.

import { childNodeIds, getAttribute, isConnected, linkedNode, setAttribute } from "../tree.mts";
import { nodeListOf } from "./collections.mts";
import { isDestroyed } from "./lifecycle.mts";
import type { NodeLink } from "../native.mts";
import type { IncaDocument } from "../types.mts";
import type { HandleBuilders, NodeMutationMembers } from "./nodeMutation.mts";
import type { NodeRelationMembers } from "./nodeRelations.mts";
import type { IncaElement, IncaFragment, IncaNode, NodeMembers } from "./types.mts";

const NODE_TYPES = { element: 1, text: 3, comment: 8, document: 9, fragment: 11 } as const;

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
 * Stores the data of a text node or comment stand-in. An element, the document and a freed node
 * ignore it.
 * @param node - the text node or comment stand-in
 * @param data - the new data
 */
export function writeData(node: IncaNode, data: string): void {
  if (node.kind === "element" || node.kind === "fragment" || node.kind === "document") return;
  if (isDestroyed(node)) return;
  setAttribute(node.__node_id, "value", data);
  if (node.kind === "text") node.text = data;
}

/**
 * Builds the node members of `node`. Each read asks the host tree again.
 * @param node - the handle the members belong to
 * @param builders - the lookups of the document handle and of a handle by node id
 * @returns the members, as getters
 */
export function nodeTreeMembers(
  node: IncaNode,
  builders: HandleBuilders,
): Omit<NodeMembers, keyof NodeMutationMembers | keyof NodeRelationMembers> {
  const id = node.__node_id;
  const reach = (link: NodeLink): IncaNode | null => {
    if (isDestroyed(node)) return null;
    const found = linkedNode(id, link);
    return found === null ? null : (builders.node(found) ?? null);
  };
  let childNodes: NodeMembers["childNodes"] | undefined;
  const liveChildren = (): IncaNode[] =>
    isDestroyed(node) ? [] : childNodeIds(id).flatMap((child) => builders.node(child) ?? []);
  return {
    get nodeType() {
      return NODE_TYPES[node.kind];
    },
    get nodeName() {
      if (node.kind === "element") return node.tagName;
      return node.kind === "fragment" ? "#document-fragment" : `#${node.kind}`;
    },
    get nodeValue() {
      return node.kind === "element" || node.kind === "fragment" || node.kind === "document"
        ? null
        : readData(node);
    },
    set nodeValue(value: string | null) {
      writeData(node, String(value ?? ""));
    },
    get isConnected() {
      return !isDestroyed(node) && isConnected(id);
    },
    get ownerDocument() {
      return node.kind === "document" ? null : builders.document();
    },
    get parentNode() {
      return reach("parent") as IncaElement | IncaFragment | IncaDocument | null;
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
