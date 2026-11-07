// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The fragment handle and `document.createDocumentFragment()`.

import { document } from "../document.mts";
import { createNode, holdNode, textContent } from "../tree.mts";
import { isDestroyed, registerNode, releaseWhenCollected, withNodeId } from "./lifecycle.mts";
import { nodeMutationMembers } from "./nodeMutation.mts";
import { nodeTreeMembers } from "./nodeTree.mts";
import { nullableText, setElementText } from "./text.mts";
import type { NodeId } from "../types.mts";
import type { IncaFragment, NodeMembers, TextContentMember } from "./types.mts";

/**
 * Builds the handle of a fragment.
 * @param id - the native node's id
 * @returns the new, empty handle
 */
export function createIncaFragment(id: NodeId): IncaFragment {
  const fields: Omit<IncaFragment, "__node_id" | keyof NodeMembers | keyof TextContentMember> = {
    kind: "fragment",
    parent: null,
    __children: [],
  };
  const fragment = withNodeId(fields as IncaFragment, id);
  const textMember: TextContentMember = {
    get textContent() {
      return isDestroyed(fragment) ? "" : textContent(id);
    },
    set textContent(value: string) {
      if (!isDestroyed(fragment)) setElementText(fragment, nullableText(value));
    },
  };
  for (const members of [nodeTreeMembers(fragment), nodeMutationMembers(fragment), textMember]) {
    Object.defineProperties(fragment, Object.getOwnPropertyDescriptors(members));
  }
  registerNode(fragment);
  return fragment;
}

/**
 * Allocates a fragment and holds it until its handle is collected.
 * @returns the new fragment
 */
function createDocumentFragment(): IncaFragment {
  const fragment = createIncaFragment(createNode("#fragment"));
  holdNode(fragment.__node_id);
  releaseWhenCollected(fragment);
  return fragment;
}

Object.defineProperty(document, "createDocumentFragment", {
  value: createDocumentFragment,
  configurable: true,
  writable: true,
});
