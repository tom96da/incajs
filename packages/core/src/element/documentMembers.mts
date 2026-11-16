// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The members of `Document`: the focused element and the creation methods.

import {
  createElement as createHostElement,
  createElementNS as createHostElementNS,
  createNode,
  focusedNode,
  linkedNode,
  nameOf,
} from "../ffi/index.mts";
import { attributeText, requireArguments } from "./arguments.mts";
import { elementOf, hold, receiverOf, DOCUMENT_RULE } from "./lifecycle.mts";
import type { IncaDocument } from "../types.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type { IncaNode } from "./types.mts";

/** The members {@link documentMembers} builds. */
export type DocumentMembers = Pick<
  IncaDocument,
  | "activeElement"
  | "createDocumentFragment"
  | "createElement"
  | "createElementNS"
  | "createTextNode"
  | "createComment"
>;

/**
 * Holds a node the script created until its handle is collected.
 * @param handle - the new, parentless handle
 * @returns `handle`
 */
function held<T extends IncaNode>(handle: T): T {
  hold(handle);
  return handle;
}

/**
 * Builds the members of `Document`. The creation methods make a host node and build its handle
 * through `builders`.
 * @param builders - the functions that build and find handles
 * @returns the members
 */
export function documentMembers(builders: HandleBuilders): DocumentMembers {
  return {
    get activeElement() {
      const state = receiverOf(this, DOCUMENT_RULE);
      const node = this as unknown as IncaNode;
      const focused = state.copy ? null : focusedNode();
      if (focused !== null) return elementOf(focused) ?? null;
      const first = linkedNode(node.__node_id, "first", true);
      const element = first === null ? undefined : builders.node(first);
      return element?.kind === "element" ? element : null;
    },
    createDocumentFragment() {
      receiverOf(this, DOCUMENT_RULE);
      return held(builders.fragment!(createNode("#fragment")));
    },
    createElement(localName: string) {
      receiverOf(this, DOCUMENT_RULE);
      requireArguments(arguments, 1, "createElement");
      const entry = createHostElement(attributeText(localName));
      return held(builders.element!(entry[0], entry[1]));
    },
    createElementNS(namespace: string | null, qualifiedName: string) {
      receiverOf(this, DOCUMENT_RULE);
      requireArguments(arguments, 2, "createElementNS");
      const space = namespace === null || namespace === undefined ? null : attributeText(namespace);
      const entry = createHostElementNS(space || null, attributeText(qualifiedName));
      return held(builders.element!(entry[0], nameOf(entry)));
    },
    createTextNode(data: string) {
      receiverOf(this, DOCUMENT_RULE);
      requireArguments(arguments, 1, "createTextNode");
      return held(builders.createText!(attributeText(data)));
    },
    createComment(data: string) {
      receiverOf(this, DOCUMENT_RULE);
      requireArguments(arguments, 1, "createComment");
      return held(builders.createComment!(attributeText(data)));
    },
  };
}
