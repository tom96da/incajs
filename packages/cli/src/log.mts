// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { styleText } from "node:util";

import { HostError } from "./dev-client/index.mts";

/** The prefix on every line the CLI prints. */
const TAG = "[inca]";

/** Shared by {@link log} and {@link printFault}. */
export interface LogOptions {
  /** Prefixes the line with `HH:MM:SS`, the way Vite's dev server stamps its own. */
  timestamp?: boolean;
}

/** Builds the line's prefix. `styleText` leaves it unstyled for a stream that can't colour. */
function tag(stream: NodeJS.WritableStream, options: LogOptions, color: "cyan" | "red"): string {
  const label = styleText(["bold", color], TAG, { stream });
  if (!options.timestamp) return label;
  const time = new Date().toLocaleTimeString("en-US", { hour12: false });
  return `${styleText("dim", time, { stream })} ${label}`;
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
}

/** Reduces a thrown value to a {@link Fault}. A `HostError` carries the host process's stack. */
export function toFault(error: unknown): Fault {
  if (error instanceof HostError) return { message: error.message, stack: error.hostStack };
  if (error instanceof Error) return { message: error.message, stack: error.stack ?? null };
  return { message: String(error), stack: null };
}

/** Writes `[inca] <label>: <message>` to `stream`, followed by the stack when there is one. */
export function printFault(
  stream: NodeJS.WritableStream,
  label: string,
  fault: Fault,
  options: LogOptions = {},
): void {
  stream.write(`${tag(stream, options, "red")} ${label}: ${fault.message}\n`);
  if (fault.stack) stream.write(`${fault.stack}\n`);
}
