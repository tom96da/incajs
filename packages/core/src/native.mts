// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { CSSStyleDeclaration, DOMTokenList } from "./element/types.mts";
import type {
  AttributeValue,
  CallbackId,
  EventLike,
  NodeId,
  ScrollAlignment,
  TagName,
} from "./types.mts";

/** How {@link IncaNative.linkedNode} names the node to reach from a node. */
export type NodeLink = "parent" | "first" | "last" | "previous" | "next";

/** The namespace of the elements `createElement()` makes. */
export const HTML_NAMESPACE = "http://www.w3.org/1999/xhtml";

/**
 * The name of an element. A string is the tag of an HTML element: it is the local name, and its
 * ASCII upper case is the `tagName`. A tuple holds the four name parts of
 * `createElementNS`.
 */
export type ElementName =
  | string
  | [localName: string, namespaceURI: string | null, prefix: string | null, tagName: string];

/** What a native returns for a node without a name: id and the marker or tag. */
export type NodeEntry = [id: NodeId, tag: string];

/** What a native returns for an element it makes: id and the four name parts. */
export type ElementEntry = [
  id: NodeId,
  localName: string,
  namespaceURI: string | null,
  prefix: string | null,
  tagName: string,
];

/**
 * One node of a parsed fragment: id, tag, namespace (`null` for a text node, a comment and a
 * fragment), parent id and the text of a text node or a comment.
 */
export type ParsedEntry = [
  id: NodeId,
  tag: string,
  namespaceURI: string | null,
  parentId: NodeId,
  data: string | null,
];

/**
 * What a parse writes into the tree: the position of the first inserted node among the children of
 * its container, and the nodes in pre-order. An entry whose parent is no entry is a root.
 */
export type Parsed = [position: number, entries: ParsedEntry[]];

/** What a listener or a dispatch names: a node id, or the tag of `window`. */
export type EventTargetId = NodeId | "$window";

/**
 * The host-injected object every wrapper in this package forwards to. Not part of the public
 * surface.
 */
interface IncaNative {
  rootNodeId(): NodeId;
  documentNodeId(): NodeId;
  createNode(tag: string): NodeId;
  createElement(localName: string): [NodeId, TagName];
  createElementNS(namespace: string | null, qualifiedName: string): ElementEntry;
  attributeKey(nodeId: NodeId, name: string): string;
  appendChild(parentId: NodeId, nodeId: NodeId): number;
  insertBefore(parentId: NodeId, nodeId: NodeId, refId: NodeId | null): number;
  removeChild(parentId: NodeId, childId: NodeId): void;
  replaceChild(parentId: NodeId, nodeId: NodeId, childId: NodeId): number;
  cloneNode(nodeId: NodeId, deep: boolean): (NodeEntry | ElementEntry)[];
  templateContent(nodeId: NodeId): NodeId | null;
  innerHtml(nodeId: NodeId): string;
  outerHtml(nodeId: NodeId): string;
  setInnerHtml(nodeId: NodeId, html: string): Parsed;
  setOuterHtml(nodeId: NodeId, html: string): Parsed | null;
  insertAdjacentHtml(nodeId: NodeId, position: string, html: string): Parsed;
  rootOf(nodeId: NodeId): NodeId;
  compareNodePosition(nodeId: NodeId, otherId: NodeId): number;
  isEqualNode(nodeId: NodeId, otherId: NodeId): boolean;
  setAttribute(nodeId: NodeId, key: string, value: AttributeValue): void;
  getAttribute(nodeId: NodeId, key: string): AttributeValue | null;
  removeAttribute(nodeId: NodeId, key: string): void;
  attributeNames(nodeId: NodeId): string[];
  classList(nodeId: NodeId): DOMTokenList;
  datasetGet(nodeId: NodeId, key: string): string | undefined;
  datasetSet(nodeId: NodeId, key: string, value: string): void;
  datasetDelete(nodeId: NodeId, key: string): boolean;
  datasetKeys(nodeId: NodeId): string[];
  datasetTarget(): object;
  linkedNode(nodeId: NodeId, link: NodeLink): NodeId | null;
  childNodeIds(nodeId: NodeId): NodeId[];
  isConnected(nodeId: NodeId): boolean;
  textContent(nodeId: NodeId): string;
  wholeText(nodeId: NodeId): string;
  substringData(nodeId: NodeId, offset: number, count: number): string;
  replaceData(nodeId: NodeId, offset: number, count: number, data: string): void;
  splitText(nodeId: NodeId, offset: number): NodeId;
  matches(nodeId: NodeId, selector: string): boolean;
  closest(nodeId: NodeId, selector: string): NodeId | null;
  checkSelector(selector: string): void;
  querySelector(nodeId: NodeId, selector: string): NodeId | null;
  querySelectorAll(nodeId: NodeId, selector: string): NodeId[];
  setStyle(nodeId: NodeId, key: string, value: AttributeValue): void;
  removeStyle(nodeId: NodeId, key: string): void;
  styleOf(nodeId: NodeId): CSSStyleDeclaration;
  inputValue(nodeId: NodeId): string;
  setInputValue(nodeId: NodeId, value: string): void;
  addEventListener(
    target: EventTargetId,
    event: string,
    callback: HostCallback,
    capture?: boolean,
    passive?: boolean,
  ): CallbackId;
  removeEventListener(target: EventTargetId, event: string, callbackId: CallbackId): boolean;
  releaseListeners(nodeId: NodeId): CallbackId[];
  holdNode(nodeId: NodeId): void;
  releaseNode(nodeId: NodeId): void;
  focusNode(nodeId: NodeId, preventScroll?: boolean): void;
  blurNode(nodeId: NodeId): void;
  focusedNode(): NodeId | null;
  layoutOf(
    nodeId: NodeId,
  ): readonly [number, number, number, number, number, number, number] | null;
  scrollOf(nodeId: NodeId): readonly [number, number, number, number] | null;
  scrollNode(nodeId: NodeId, left: number | null, top: number | null): void;
  scrollIntoViewNode(nodeId: NodeId, block: ScrollAlignment, inline: ScrollAlignment): void;
  dispatchEvent(target: EventTargetId, event: EventLike): boolean;
  click(nodeId: NodeId): void;
  setPointerCapture(nodeId: NodeId, pointerId: number): void;
  releasePointerCapture(nodeId: NodeId, pointerId: number): void;
  hasPointerCapture(nodeId: NodeId, pointerId: number): boolean;
}

/** What the host calls: it passes the event object as the first argument. */
export type HostCallback = (...args: unknown[]) => void;

declare global {
  // eslint-disable-next-line no-var
  var __inca_native__: IncaNative;
  // eslint-disable-next-line no-var
  var __inca_released__: (nodeIds: NodeId[], callbackIds: CallbackId[]) => void;
  // eslint-disable-next-line no-var
  var __inca_targets__: (targets: EventTargetId[]) => unknown[];
}

export function native(): IncaNative {
  return globalThis.__inca_native__;
}
