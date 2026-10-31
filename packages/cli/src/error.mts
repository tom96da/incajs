// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/**
 * A failure the CLI raises itself, identified by an `ERR_INCA_*` code the
 * way Node's own errors carry one. The code is what a user searches for
 * and what `printFault` names alongside the message, so every distinct
 * cause gets its own.
 */
export class IncaError extends Error {
  override readonly name = "IncaError";

  constructor(
    readonly code: string,
    message: string,
    /** A file whose creation or change may resolve the error. */
    readonly path?: string,
  ) {
    super(message);
  }
}

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
