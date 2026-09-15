// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The subset of incajs's own API this renderer drives the native tree
// through, injected rather than imported directly: this package works
// against anything with this shape, and never depends on `incajs` itself
// — which matters because `incajs`'s own `incajs/vue` convenience subpath
// depends on this package, and a dependency back the other way would be
// circular.

/** Stable handle to a node in the native retained tree. */
export type NodeId = number;

/** A value a native attribute/style call can take. */
export type AttributeValue = string | number | boolean;

/** Element kind passed to `createNode`. */
export type TagName = "text" | (string & {});

/** Signature of a callback registered via `setEventListener`. */
export type EventListener = (...args: unknown[]) => void;

/**
 * The incajs functions this renderer calls, as property-typed function
 * fields rather than method signatures — the latter are bivariantly
 * checked and trip lint rules against passing them around unbound, neither
 * of which this interface needs to tolerate.
 */
export interface IncajsCore {
  rootNodeId: () => NodeId;
  createNode: (tag: TagName) => NodeId;
  appendChild: (parentId: NodeId, childId: NodeId) => void;
  insertBefore: (parentId: NodeId, childId: NodeId, anchorId: NodeId | null) => void;
  removeChild: (parentId: NodeId, childId: NodeId) => void;
  destroyNode: (nodeId: NodeId) => void;
  setAttribute: (nodeId: NodeId, key: string, value: AttributeValue) => void;
  setStyle: (nodeId: NodeId, key: string, value: AttributeValue) => void;
  setEventListener: (nodeId: NodeId, event: string, listener: EventListener) => void;
  removeEventListener: (nodeId: NodeId, event: string) => void;
}
