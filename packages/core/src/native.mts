// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { AttributeValue, CallbackId, EventListener, NodeId } from "./types.mts";

/** The host-injected object every wrapper in this package forwards to. Not part of the public surface — reach it through the wrappers instead. */
interface GpjsuiNative {
  rootNodeId(): NodeId;
  createNode(tag: string): NodeId;
  appendChild(parentId: NodeId, childId: NodeId): void;
  insertBefore(parentId: NodeId, childId: NodeId, anchorId: NodeId | null): void;
  removeChild(parentId: NodeId, childId: NodeId): void;
  setAttribute(nodeId: NodeId, key: string, value: AttributeValue): void;
  setStyle(nodeId: NodeId, key: string, value: AttributeValue): void;
  addEventListener(nodeId: NodeId, event: string, callbackId: CallbackId): void;
  removeEventListener(nodeId: NodeId, event: string, callbackId: CallbackId): boolean;
  destroyNode(nodeId: NodeId): CallbackId[];
}

declare global {
  // eslint-disable-next-line no-var
  var __gpjsui_native__: GpjsuiNative;
  // eslint-disable-next-line no-var
  var __gpjsui_callbacks__: Record<CallbackId, EventListener>;
}

export function native(): GpjsuiNative {
  return globalThis.__gpjsui_native__;
}
