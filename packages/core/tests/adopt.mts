// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { adoptHandle } from "../src/element/handle.mts";
import { declareNode } from "./setup.mts";
import type {
  Comment,
  DocumentFragment,
  Element,
  HTMLElement,
  Text,
} from "../src/element/types.mts";
import type { ElementName } from "../src/ffi/index.mts";
import type { Document } from "../src/types.mts";

/**
 * Builds the handle of an element node through the stand-in `nodeHandle`, which names it as given.
 * @param id - the node id
 * @param name - the element name, an HTML `div` when omitted
 * @param passive - the event names whose listeners are passive by default
 * @returns the handle
 */
export function adoptElement(id: number, name?: string, passive?: readonly string[]): HTMLElement;
export function adoptElement(id: number, name: ElementName, passive?: readonly string[]): Element;
export function adoptElement(
  id: number,
  name: ElementName = "div",
  passive?: readonly string[],
): Element {
  declareNode(id, { kind: "element", name });
  return adoptHandle(id, passive) as Element;
}

/**
 * Builds the handle of a text node.
 * @param id - the node id
 * @param data - the data of the node
 * @returns the handle
 */
export function adoptText(id: number, data: string): Text {
  declareNode(id, { kind: "text" });
  return adoptHandle(id, undefined, data) as Text;
}

/**
 * Builds the handle of a comment.
 * @param id - the node id
 * @returns the handle
 */
export function adoptComment(id: number): Comment {
  declareNode(id, { kind: "comment" });
  return adoptHandle(id) as Comment;
}

/**
 * Builds the handle of a fragment.
 * @param id - the node id
 * @returns the handle
 */
export function adoptFragment(id: number): DocumentFragment {
  declareNode(id, { kind: "fragment" });
  return adoptHandle(id) as DocumentFragment;
}

/**
 * Builds the handle of a copy of the document.
 * @param id - the node id
 * @returns the handle
 */
export function adoptDocument(id: number): Document {
  declareNode(id, { kind: "document" });
  return adoptHandle(id) as Document;
}
