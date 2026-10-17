// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { setTimeout as sleep } from "node:timers/promises";

import { vi } from "vitest";

/**
 * Waits out the latency of macOS FSEvents, which `fs.watch` uses there.
 * FSEvents reports a change made up to 50 ms before a watch starts, and can miss one made right after.
 * Call it before and after arming a watch.
 * It resolves at once on other platforms and under fake timers.
 */
export async function fsLag(): Promise<void> {
  if (process.platform === "darwin" && !vi.isFakeTimers()) await sleep(50);
}
