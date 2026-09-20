// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { styleText } from "node:util";

import { HostError } from "./dev-client/index.mts";

/** The prefix on every line the CLI prints. */
const TAG = "[inca]";

/** The clock Vite stamps its dev-server lines with: the viewer's own locale. */
const TIME = new Intl.DateTimeFormat(undefined, {
  hour: "numeric",
  minute: "numeric",
  second: "numeric",
});

/** Shared by {@link log} and {@link printFault}. */
export interface LogOptions {
  /** Prefixes the line with `HH:MM:SS`, the way Vite's dev server stamps its own. */
  timestamp?: boolean;
}

/** Builds the line's prefix. `styleText` leaves it unstyled for a stream that can't colour. */
function tag(stream: NodeJS.WritableStream, options: LogOptions, color: "cyan" | "red"): string {
  const label = styleText(["bold", color], TAG, { stream });
  if (!options.timestamp) return label;
  return `${styleText("dim", TIME.format(new Date()), { stream })} ${label}`;
}

/** Writes one `[inca] <message>` line to `stream`. */
export function log(
  stream: NodeJS.WritableStream,
  message: string,
  options: LogOptions = {},
): void {
  stream.write(`${tag(stream, options, "cyan")} ${message}\n`);
}

/** A failure reduced to the message and stack worth printing. */
export interface Fault {
  message: string;
  stack: string | null;
  /** An `ERR_INCA_*` identifier, when the failure has one to look up. */
  code?: string | null;
}

/** The `code` an `Error` carries, in the shape Node's own errors use. */
function codeOf(error: Error): string | null {
  const code: unknown = (error as { code?: unknown }).code;
  return typeof code === "string" ? code : null;
}

/**
 * The code read back out of a message. A bundler re-wraps a plugin's error
 * and keeps only its text, so the code is repeated there for this to find.
 */
function codeIn(message: string): string | null {
  return /\bERR_INCA_[A-Z0-9_]+/.exec(message)?.[0] ?? null;
}

/**
 * Drops the header `Error.stack` repeats — Node writes `Name: message`, a
 * bundler's aggregated error the bare message. A stack in any other shape
 * is kept whole.
 */
function framesOf(error: Error): string | null {
  const stack = error.stack;
  if (!stack) return null;
  for (const header of [`${error.name}: ${error.message}`, error.message]) {
    if (stack.startsWith(header)) return stack.slice(header.length).replace(/^\r?\n/, "") || null;
  }
  return stack;
}

/**
 * Reduces an error a bundler re-wrapped: its text is kept whole — the
 * excerpt a compiler points at is the useful part — while the error class
 * and any `ERR_INCA_*` marker are lifted off the first line, the latter
 * into the fault's code.
 */
export function bundlerFault(message: string): Fault {
  const marked = /^(?:\w*Error: )?(ERR_INCA_[A-Z0-9_]+): /.exec(message);
  if (marked) return { message: message.slice(marked[0].length), stack: null, code: marked[1] };
  return { message: message.replace(/^\w*Error: /, ""), stack: null, code: null };
}

/** Reduces a thrown value to a {@link Fault}. A `HostError` carries the host process's stack. */
export function toFault(error: unknown): Fault {
  if (error instanceof HostError) {
    return { message: error.message, stack: error.hostStack, code: codeOf(error) };
  }
  if (error instanceof Error) {
    return {
      message: error.message,
      stack: framesOf(error),
      code: codeOf(error) ?? codeIn(error.message),
    };
  }
  return { message: String(error), stack: null, code: null };
}

/**
 * Writes `[inca] <label> (<code>): <message>` to `stream` — the code only
 * when the failure carries one — followed by the stack when there is one.
 */
export function printFault(
  stream: NodeJS.WritableStream,
  label: string,
  fault: Fault,
  options: LogOptions = {},
): void {
  const code = fault.code ? ` (${styleText("bold", fault.code, { stream })})` : "";
  stream.write(`${tag(stream, options, "red")} ${label}${code}: ${fault.message}\n`);
  if (fault.stack) stream.write(`${fault.stack}\n`);
}
