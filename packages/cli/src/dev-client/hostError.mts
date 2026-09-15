// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/** A fault the host answered a request with, carrying its JSON-RPC code. */
export class HostError extends Error {
  constructor(
    message: string,
    readonly code: number,
    readonly hostStack: string | null = null,
  ) {
    super(message);
    this.name = "HostError";
  }
}
