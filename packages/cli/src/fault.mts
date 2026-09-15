// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { HostError } from "./dev-client/index.mts";

export interface Fault {
  message: string;
  stack: string | null;
}

export function toFault(error: unknown): Fault {
  if (error instanceof HostError) return { message: error.message, stack: error.hostStack };
  if (error instanceof Error) return { message: error.message, stack: error.stack ?? null };
  return { message: String(error), stack: null };
}

export function printFault(stream: NodeJS.WritableStream, label: string, fault: Fault): void {
  stream.write(`[inca] ${label}: ${fault.message}\n`);
  if (fault.stack) stream.write(`${fault.stack}\n`);
}
