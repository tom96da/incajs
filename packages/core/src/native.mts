// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { AttributeValue, CallbackId, EventLike, NodeId } from "./types.mts";

/** The host-injected object every wrapper in this package forwards to. Not part of the public surface — reach it through the wrappers instead. */
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
  addEventListener(
    nodeId: NodeId,
    event: string,
    callbackId: CallbackId,
    capture?: boolean,
    passive?: boolean,
  ): void;
  removeEventListener(nodeId: NodeId, event: string, callbackId: CallbackId): boolean;
  destroyNode(nodeId: NodeId): CallbackId[];
  focusNode(nodeId: NodeId): void;
  blurNode(nodeId: NodeId): void;
  focusedNode(): NodeId | null;
  layoutOf(
    nodeId: NodeId,
  ): readonly [number, number, number, number, number, number, number] | null;
  scrollOf(nodeId: NodeId): readonly [number, number, number, number] | null;
  scrollNode(nodeId: NodeId, left: number | null, top: number | null): void;
  dispatchEvent(nodeId: NodeId, event: EventLike): boolean;
  click(nodeId: NodeId): void;
}

/** What the host calls: it passes the event object as the first argument. */
export type HostCallback = (...args: unknown[]) => void;

declare global {
  // eslint-disable-next-line no-var
  var __inca_native__: IncaNative;
  // eslint-disable-next-line no-var
  var __inca_callbacks__: Record<CallbackId, HostCallback>;
}

export function native(): IncaNative {
  return globalThis.__inca_native__;
}
