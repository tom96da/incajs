// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The members of text nodes that take or return a handle, and the renderer's text update.

import { setCharacterData, splitText } from "../ffi/index.mts";
import { requireArguments } from "./arguments.mts";
import { isDestroyed, TEXT_RULE, receiverNode } from "./lifecycle.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type { TextMembers } from "./nodeTypes.mts";
import type { Text } from "./types.mts";

/**
 * Builds the members of a text node that return a handle. The host defines `wholeText` and the
 * `CharacterData` members.
 * @param builders - the lookup that builds the handle of the new node
 * @returns the members
 */
export function textMembers(builders: HandleBuilders): Pick<TextMembers, "splitText"> {
  return {
    splitText(offset: number) {
      const node = receiverNode<Text>(this, TEXT_RULE);
      requireArguments(arguments, 1, "splitText");
      if (isDestroyed(node)) throw new TypeError("the text node was freed. create a new one");
      return builders.node(splitText(node.__node_id, offset)) as Text;
    },
  };
}

/**
 * Updates a text leaf's content in place.
 * @param node - the text leaf to update
 * @param text - the new text content
 */
export function setText(node: Text, text: string): void {
  setCharacterData(node.__node_id, text);
}
