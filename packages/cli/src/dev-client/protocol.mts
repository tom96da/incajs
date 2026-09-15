// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The wire shapes this package reads off the host's stdout. Internal only —
// `HostClient` is what a caller talks to; nothing here is re-exported from
// `index.mts`.

export const JSONRPC = "2.0";

/**
 * The host method-set revision this package expects, checked against
 * `ready`'s `params.protocol` — a mismatch means an incompatible host
 * build.
 */
export const HOST_PROTOCOL_VERSION = 0;

/** `ready`'s notification params. */
export interface ReadyParams {
  protocol: number;
}

/** `appError`'s notification params. */
export interface AppErrorParams {
  message: string;
  stack: string | null;
}

export interface RpcResult {
  jsonrpc: typeof JSONRPC;
  id: number;
  result: unknown;
}

export interface RpcFailure {
  jsonrpc: typeof JSONRPC;
  id: number;
  error: { code: number; message: string; data?: { stack: string | null } };
}

export interface RpcNotification {
  jsonrpc: typeof JSONRPC;
  method: string;
  params: unknown;
}

export function isRpcMessage(value: unknown): value is RpcResult | RpcFailure | RpcNotification {
  return (
    typeof value === "object" &&
    value !== null &&
    (value as { jsonrpc?: unknown }).jsonrpc === JSONRPC
  );
}
