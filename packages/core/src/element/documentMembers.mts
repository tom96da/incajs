// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The members of `Document`: the focused element and the creation methods.

import {
  createElement as createHostElement,
  createElementNS as createHostElementNS,
  createNode,
  focusedNode,
  linkedNode,
  setCharacterData,
} from "../ffi/index.mts";
import { attributeText, requireArguments } from "./arguments.mts";
import { hold, receiverOf, DOCUMENT_RULE } from "./lifecycle.mts";
import type { Document } from "../types.mts";
import type { HandleNode } from "./handleNode.mts";
import type { HandleBuilders } from "./lifecycle.mts";

/** The members {@link documentMembers} builds. */
export type DocumentMembers = Pick<
  Document,
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
function held<T extends HandleNode>(handle: HandleNode | undefined): T {
  hold(handle!);
  return handle as T;
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
      const node = this as unknown as HandleNode;
      const focused = state.copy ? null : focusedNode();
      const id = focused ?? linkedNode(node.__node_id, "first", true);
      const element = id === null ? undefined : builders.node(id);
      return element?.kind === "element" ? element : null;
    },
    createDocumentFragment() {
      receiverOf(this, DOCUMENT_RULE);
      return held(builders.adopt(createNode("#fragment")));
    },
    createElement(localName: string) {
      receiverOf(this, DOCUMENT_RULE);
      requireArguments(arguments, 1, "createElement");
      const entry = createHostElement(attributeText(localName));
      return held(builders.adopt(entry[0]));
    },
    createElementNS(namespace: string | null, qualifiedName: string) {
      receiverOf(this, DOCUMENT_RULE);
      requireArguments(arguments, 2, "createElementNS");
      const space = namespace === null || namespace === undefined ? null : attributeText(namespace);
      const entry = createHostElementNS(space || null, attributeText(qualifiedName));
      return held(builders.adopt(entry[0]));
    },
    createTextNode(data: string) {
      receiverOf(this, DOCUMENT_RULE);
      requireArguments(arguments, 1, "createTextNode");
      const id = createNode("#text");
      const text = attributeText(data);
      setCharacterData(id, text);
      return held(builders.adopt(id));
    },
    createComment(data: string) {
      receiverOf(this, DOCUMENT_RULE);
      requireArguments(arguments, 1, "createComment");
      const id = createNode("#comment");
      setCharacterData(id, attributeText(data));
      return held(builders.adopt(id));
    },
  };
}
