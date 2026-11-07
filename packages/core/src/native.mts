// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { AttributeValue, CallbackId, EventLike, NodeId, ScrollAlignment } from "./types.mts";

/** How {@link IncaNative.linkedNode} names the node to reach from a node. */
export type NodeLink = "parent" | "first" | "last" | "previous" | "next";

/** What a listener or a dispatch names: a node id, or the tag of `document` or `window`. */
export type EventTargetId = NodeId | "$document" | "$window";

/**
 * The host-injected object every wrapper in this package forwards to. Not part of the public
 * surface.
 */
interface IncaNative {
  rootNodeId(): NodeId;
  createNode(tag: string): NodeId;
  appendChild(parentId: NodeId, nodeId: NodeId): number;
  insertBefore(parentId: NodeId, nodeId: NodeId, refId: NodeId | null): number;
  removeChild(parentId: NodeId, childId: NodeId): void;
  replaceChild(parentId: NodeId, nodeId: NodeId, childId: NodeId): number;
  setAttribute(nodeId: NodeId, key: string, value: AttributeValue): void;
  getAttribute(nodeId: NodeId, key: string): AttributeValue | null;
  removeAttribute(nodeId: NodeId, key: string): void;
  attributeNames(nodeId: NodeId): string[];
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
