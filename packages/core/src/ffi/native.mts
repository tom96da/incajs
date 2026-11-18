// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { CSSStyleDeclaration, DOMTokenList } from "../element/types.mts";
import type { EventLike, TagName } from "../types.mts";

/**
 * Stable handle to a node in the native retained tree, returned by `createNode`
 * and used in every later call that touches that node.
 */
export type NodeId = number;

/**
 * Id assigned to one registered event listener. Managed internally by
 * `addEventListener`/`removeEventListener`/`removeNode`.
 */
export type CallbackId = number;

/**
 * A value `setAttribute`/`setStyle` can take. Any other value raises a catchable
 * exception.
 */
export type AttributeValue = string | number | boolean;

/** Where `scrollIntoView` aligns a node on one axis. */
export type ScrollAlignment = "start" | "center" | "end" | "nearest";

/** How {@link NativeBindings.linkedNode} names the node to reach from a node. */
export type NodeLink = "parent" | "first" | "last" | "previous" | "next";

/** Where `insertNodes` puts its nodes: into the node or next to it. */
export type InsertionPlace =
  | "append"
  | "prepend"
  | "before"
  | "after"
  | "replaceWith"
  | "replaceChildren";

/**
 * The subtrees a call left without a parent: one array for each such node, with the ids of its
 * subtree in pre-order and the node first.
 */
export type RemovedSubtrees = NodeId[][];

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

/**
 * The last drawn box of a node in window logical pixels. `border` is one uniform width. `scrollX`
 * and `scrollY` are the positive scroll offset the parent applied to the box, and `gutterWidth`
 * and `gutterHeight` are the strips the scrollbars reserve inside the border.
 */
export type NodeBox = readonly [
  x: number,
  y: number,
  width: number,
  height: number,
  border: number,
  scrollX: number,
  scrollY: number,
  gutterWidth: number,
  gutterHeight: number,
];

/** The scroll offset of a node and its maximum, as of the last drawn frame. */
export type NodeScroll = readonly [left: number, top: number, maxLeft: number, maxTop: number];

/** What a native returns for an element it makes: id and the four name parts. */
export type ElementEntry = [
  id: NodeId,
  localName: string,
  namespaceURI: string | null,
  prefix: string | null,
  tagName: string,
];

/**
 * What a native `nodeHandle` returns for a node: the handle, whether the node has a parent and the
 * special element it is.
 */
export type NodeHandleResult = [
  handle: object,
  parented: boolean,
  special: "input" | "template" | null,
];

/** The target that names `window`, as the host gives it. */
export type WindowTargetId = string & { readonly __windowTarget: unique symbol };

/** An `AbortSignal` object that names itself as a target. */
export type SignalTarget = object & { readonly __signalTarget: unique symbol };

/** What a listener or a dispatch names: a node id, the target of `window`, or a signal. */
export type EventTargetId = NodeId | WindowTargetId | SignalTarget;

/** What `__inca_targets__` takes: the targets that have no signal. */
export type HookTarget = NodeId | WindowTargetId;

/**
 * The prototypes of the node interfaces that the host holds, by name. `ForeignElement` is the
 * prototype of an SVG or MathML element and no global names it.
 */
export interface InterfacePrototypes {
  EventTarget: object;
  Node: object;
  CharacterData: object;
  Text: object;
  Comment: object;
  Element: object;
  HTMLElement: object;
  ForeignElement: object;
  DocumentFragment: object;
  Document: object;
  HTMLDocument: object;
}

/**
 * The host-injected object every wrapper in this package forwards to. Not part of the public
 * surface.
 */
interface NativeBindings {
  rootNodeId(): NodeId;
  documentNodeId(): NodeId;
  interfacePrototypes(): InterfacePrototypes;
  registerUncloneable(map: WeakMap<object, unknown>): void;
  createNode(tag: string): NodeId;
  createElement(localName: string): [NodeId, TagName];
  createElementNS(namespace: string | null, qualifiedName: string): ElementEntry;
  attributeKey(nodeId: NodeId, name: string): string;
  appendChild(parentId: NodeId, nodeId: NodeId): number;
  insertBefore(parentId: NodeId, nodeId: NodeId, refId: NodeId | null): number;
  removeChild(parentId: NodeId, childId: NodeId): RemovedSubtrees;
  replaceChild(parentId: NodeId, nodeId: NodeId, childId: NodeId): RemovedSubtrees;
  replaceWithText(nodeId: NodeId, text: string): [textId: NodeId | null, removed: RemovedSubtrees];
  insertNodes(
    nodeId: NodeId,
    place: InsertionPlace,
    nodes: (NodeId | string)[],
  ): [textIds: NodeId[], removed: RemovedSubtrees] | null;
  cloneNode(nodeId: NodeId, deep: boolean): NodeId;
  nodeHandle(nodeId: NodeId): NodeHandleResult | null;
  templateContent(nodeId: NodeId): NodeId | null;
  setInnerHtml(nodeId: NodeId, html: string): RemovedSubtrees;
  setOuterHtml(nodeId: NodeId, html: string): RemovedSubtrees;
  insertAdjacentHtml(nodeId: NodeId, position: string, html: string): void;
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
  linkedNode(nodeId: NodeId, link: NodeLink, elementOnly?: boolean): NodeId | null;
  childNodeIds(nodeId: NodeId, elementOnly?: boolean): NodeId[];
  childCount(nodeId: NodeId, elementOnly: boolean): number;
  childAt(nodeId: NodeId, index: number, elementOnly: boolean): NodeId | null;
  namedChild(nodeId: NodeId, name: string): NodeId | null;
  childNames(nodeId: NodeId): string[];
  textContent(nodeId: NodeId): string;
  setCharacterData(nodeId: NodeId, data: string): void;
  splitText(nodeId: NodeId, offset: number): NodeId;
  closest(nodeId: NodeId, selector: string): NodeId | null;
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
    signal?: SignalTarget,
  ): CallbackId | null;
  removeEventListener(target: EventTargetId, event: string, callbackId: CallbackId): boolean;
  removeNode(nodeId: NodeId): [callbackIds: CallbackId[], nodeIds: NodeId[]];
  holdNode(nodeId: NodeId): void;
  releaseNode(nodeId: NodeId): void;
  focusNode(nodeId: NodeId, preventScroll?: boolean): void;
  blurNode(nodeId: NodeId): void;
  focusedNode(): NodeId | null;
  layoutOf(nodeId: NodeId): NodeBox | null;
  scrollOf(nodeId: NodeId): NodeScroll | null;
  scrollNode(nodeId: NodeId, left: number | null, top: number | null): void;
  scrollIntoViewNode(nodeId: NodeId, block: ScrollAlignment, inline: ScrollAlignment): void;
  dispatchEvent(target: EventTargetId, event: EventLike): boolean;
  windowTargetId(): WindowTargetId;
  signalId(value: unknown): number | null;
  click(nodeId: NodeId): void;
}

/** What the host calls: it passes the event object as the first argument. */
export type HostCallback = (...args: unknown[]) => void;

declare global {
  // eslint-disable-next-line no-var
  var __inca_native__: NativeBindings;
  // eslint-disable-next-line no-var
  var __inca_released__: (nodeIds: NodeId[], callbackIds: CallbackId[]) => void;
  // eslint-disable-next-line no-var
  var __inca_targets__: (targets: HookTarget[]) => unknown[];
}

export function native(): NativeBindings {
  return globalThis.__inca_native__;
}
