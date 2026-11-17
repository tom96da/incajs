// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The text handle, and the members of character data and of text nodes.

import {
  linkedNode,
  replaceData,
  setCharacterData,
  splitText,
  substringData,
  textContent,
  wholeText,
} from "../ffi/index.mts";
import { attributeText, requireArguments } from "./arguments.mts";
import {
  entered,
  hold,
  isDestroyed,
  prototypeOf,
  registerNode,
  NO_EVENTS,
  withState,
  CHARACTER_DATA_RULE,
  TEXT_RULE,
  receiverNode,
} from "./lifecycle.mts";
import { readData, writeData } from "./nodeTree.mts";
import type { NodeId } from "../types.mts";
import type { HandleNode } from "./handleNode.mts";
import type { CharacterData, Node, TextMembers } from "./nodeTypes.mts";
import type { Text } from "./types.mts";

/**
 * Builds the members of `CharacterData`, which read and edit the data of a text node or a
 * comment.
 * @returns the members
 */
export function characterDataMembers(): Omit<CharacterData, keyof Node> {
  const nodeOf = (self: unknown): HandleNode => receiverNode(self, CHARACTER_DATA_RULE);
  /** Replaces a range of the data. A freed node ignores the call. */
  const edit = (node: HandleNode, offset: number, count: number, data: string): void => {
    if (isDestroyed(node)) return;
    replaceData(node.__node_id, offset, count, data);
    if (node.kind === "text") node.text = readData(node);
  };
  return {
    get data() {
      return readData(nodeOf(this));
    },
    set data(value: string) {
      writeData(nodeOf(this), value === null ? "" : attributeText(value));
    },
    get length() {
      return readData(nodeOf(this)).length;
    },
    substringData(offset: number, count: number) {
      const node = nodeOf(this);
      requireArguments(arguments, 2, "substringData");
      if (!isDestroyed(node)) return substringData(node.__node_id, offset, count);
      const start = offset >>> 0;
      return readData(node).slice(start, start + (count >>> 0));
    },
    appendData(data: string) {
      const node = nodeOf(this);
      requireArguments(arguments, 1, "appendData");
      edit(node, readData(node).length, 0, attributeText(data));
    },
    insertData(offset: number, data: string) {
      const node = nodeOf(this);
      requireArguments(arguments, 2, "insertData");
      edit(node, offset, 0, attributeText(data));
    },
    deleteData(offset: number, count: number) {
      const node = nodeOf(this);
      requireArguments(arguments, 2, "deleteData");
      edit(node, offset, count, "");
    },
    replaceData(offset: number, count: number, data: string) {
      const node = nodeOf(this);
      requireArguments(arguments, 3, "replaceData");
      edit(node, offset, count, attributeText(data));
    },
  };
}

/**
 * Builds the members only a text node has.
 * @returns the members
 */
export function textMembers(): TextMembers {
  return {
    get wholeText() {
      const node = receiverNode<Text>(this, TEXT_RULE);
      return isDestroyed(node) ? node.text : wholeText(node.__node_id);
    },
    splitText(offset: number) {
      const node = receiverNode<Text>(this, TEXT_RULE);
      requireArguments(arguments, 1, "splitText");
      if (isDestroyed(node)) throw new TypeError("the text node was freed. create a new one");
      const tailId = splitText(node.__node_id, offset);
      const tail = createIncaText(tailId, textContent(tailId));
      node.text = readData(node);
      if (linkedNode(tailId, "parent") === null) hold(tail);
      else entered(tail);
      return tail;
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
  node.text = text;
}

/**
 * Builds the handle of a text node.
 * @param id - the native node's id
 * @param text - the data the node holds
 * @returns the new, parentless handle
 */
export function createIncaText(id: NodeId, text: string): Text {
  const handle = Object.create(prototypeOf("text")) as Text;
  withState(handle, id, "text", NO_EVENTS);
  handle.text = text;
  registerNode(handle);
  return handle;
}
