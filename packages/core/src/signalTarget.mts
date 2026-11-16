// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { signalId } from "./ffi/index.mts";
import type { SignalTarget } from "./ffi/index.mts";

/**
 * Gives `value` as a target when it is an `AbortSignal` of the host.
 * @param value - any value
 * @returns the signal as a target, or `undefined` for any other value
 */
export function signalTargetOf(value: unknown): SignalTarget | undefined {
  return signalId(value) === null ? undefined : (value as SignalTarget);
}
