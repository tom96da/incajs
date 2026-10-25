// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { AttributeValue, CallbackId, EventLike, NodeId, ScrollAlignment } from "./types.mts";

/** What a listener or a dispatch names: a node id, or the tag of `document` or `window`. */
export type EventTargetId = NodeId | "$document" | "$window";

/**
 * The host-injected object every wrapper in this package forwards to. Not part of the public
 * surface.
 */
interface IncaNative {
  rootNodeId(): NodeId;
  createNode(tag: string): NodeId;
  appendChild(parentId: NodeId, childId: NodeId): void;
  insertBefore(parentId: NodeId, childId: NodeId, anchorId: NodeId | null): void;
  removeChild(parentId: NodeId, childId: NodeId): void;
  setAttribute(nodeId: NodeId, key: string, value: AttributeValue): void;
  getAttribute(nodeId: NodeId, key: string): AttributeValue | null;
  removeAttribute(nodeId: NodeId, key: string): void;
  setStyle(nodeId: NodeId, key: string, value: AttributeValue): void;
  removeStyle(nodeId: NodeId, key: string): void;
  inputValue(nodeId: NodeId): string;
  setInputValue(nodeId: NodeId, value: string): void;
  addEventListener(
    target: EventTargetId,
    event: string,
    capture?: boolean,
    passive?: boolean,
  ): CallbackId;
  removeEventListener(target: EventTargetId, event: string, callbackId: CallbackId): boolean;
  destroyNode(nodeId: NodeId): CallbackId[];
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
  var __inca_callbacks__: Record<CallbackId, HostCallback>;
  // eslint-disable-next-line no-var
  var __inca_released__: (nodeIds: NodeId[], callbackIds: CallbackId[]) => void;
  // eslint-disable-next-line no-var
  var __inca_targets__: (targets: EventTargetId[]) => unknown[];
}

export function native(): IncaNative {
  return globalThis.__inca_native__;
}
