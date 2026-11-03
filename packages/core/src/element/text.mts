// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The text handle, and the character data members of text nodes and comment stand-ins.

import {
  getAttribute,
  holdNode,
  replaceData,
  splitText,
  substringData,
  wholeText,
} from "../tree.mts";
import { attributeText, requireArguments } from "./attributes.mts";
import { isDestroyed, registerNode, releaseWhenCollected, withNodeId } from "./lifecycle.mts";
import { nodeTreeMembers, readData, writeData } from "./nodeTree.mts";
import type { NodeId } from "../types.mts";
import type {
  CharacterDataMembers,
  IncaNode,
  IncaText,
  NodeMembers,
  TextMembers,
} from "./types.mts";

/** Converts an assignment to a nullable text member: `null` and `undefined` store `""`. */
export function nullableText(value: unknown): string {
  return value === null || value === undefined ? "" : attributeText(value);
}

/**
 * Builds the character data members of a text node or a comment stand-in.
 * @param node - the handle the members belong to
 * @returns the members
 */
export function characterDataMembers(node: IncaNode): CharacterDataMembers {
  const id = node.__node_id;
  /** Replaces a range of the data. A freed node ignores the call. */
  const edit = (offset: number, count: number, data: string): void => {
    if (isDestroyed(node)) return;
    replaceData(id, offset, count, data);
    if (node.kind === "text") node.text = readData(node);
  };
  return {
    get data() {
      return readData(node);
    },
    set data(value: string) {
      writeData(node, value === null ? "" : attributeText(value));
    },
    get length() {
      return readData(node).length;
    },
    get textContent() {
      return readData(node);
    },
    set textContent(value: string) {
      writeData(node, nullableText(value));
    },
    substringData: (...args: [offset: number, count: number]) => {
      requireArguments(args, 2, "substringData");
      if (!isDestroyed(node)) return substringData(id, args[0], args[1]);
      const start = args[0] >>> 0;
      return readData(node).slice(start, start + (args[1] >>> 0));
    },
    appendData: (...args: [data: string]) => {
      requireArguments(args, 1, "appendData");
      edit(readData(node).length, 0, attributeText(args[0]));
    },
    insertData: (...args: [offset: number, data: string]) => {
      requireArguments(args, 2, "insertData");
      edit(args[0], 0, attributeText(args[1]));
    },
    deleteData: (...args: [offset: number, count: number]) => {
      requireArguments(args, 2, "deleteData");
      edit(args[0], args[1], "");
    },
    replaceData: (...args: [offset: number, count: number, data: string]) => {
      requireArguments(args, 3, "replaceData");
      edit(args[0], args[1], attributeText(args[2]));
    },
  };
}

/**
 * Builds the members only a text node has.
 * @param node - the text handle the members belong to
 * @returns the members
 */
function textMembers(node: IncaText): TextMembers {
  const id = node.__node_id;
  return {
    get wholeText() {
      return isDestroyed(node) ? node.text : wholeText(id);
    },
    splitText: (...args: [offset: number]) => {
      requireArguments(args, 1, "splitText");
      if (isDestroyed(node)) throw new TypeError("the text node was freed. create a new one");
      const tailId = splitText(id, args[0]);
      const tail = createIncaText(tailId, String(getAttribute(tailId, "value") ?? ""));
      node.text = readData(node);
      const parent = node.parent;
      if (parent === null) {
        holdNode(tail.__node_id);
        releaseWhenCollected(tail);
      } else {
        parent.__children.splice(parent.__children.indexOf(node) + 1, 0, tail);
        tail.parent = parent;
      }
      return tail;
    },
  };
}

/**
 * Builds the handle of a text node.
 * @param id - the native node's id
 * @param text - the data the node holds
 * @returns the new, parentless handle
 */
export function createIncaText(id: NodeId, text: string): IncaText {
  const node: Omit<
    IncaText,
    "__node_id" | keyof NodeMembers | keyof CharacterDataMembers | keyof TextMembers
  > = {
    kind: "text",
    parent: null,
    text,
  };
  const handle = withNodeId(node as IncaText, id);
  for (const members of [
    nodeTreeMembers(handle),
    characterDataMembers(handle),
    textMembers(handle),
  ]) {
    Object.defineProperties(handle, Object.getOwnPropertyDescriptors(members));
  }
  registerNode(handle);
  return handle;
}
