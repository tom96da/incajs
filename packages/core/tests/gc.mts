// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { setFlagsFromString } from "node:v8";
import { runInNewContext } from "node:vm";

const bun = (globalThis as { Bun?: { gc(sync: boolean): void } }).Bun;
if (!bun) setFlagsFromString("--expose-gc");
const gc = bun ? () => bun.gc(true) : (runInNewContext("gc") as () => void);

/** The rounds an unconditional collection runs. */
const FIXED_ROUNDS = bun ? 50 : 10;

/** The rounds a collection waits for its condition. */
const ROUND_LIMIT = 200;

/**
 * Runs collection rounds. Each round runs in its own timer tick, since a `deref` keeps its target
 * alive until the job ends, and checks the condition after the collector ran.
 * @param until - a condition that ends the rounds when it holds. Without it the collection runs a
 *   fixed number of rounds.
 */
export async function collectGarbage(until?: () => boolean): Promise<void> {
  for (let round = 0; round < (until ? ROUND_LIMIT : FIXED_ROUNDS); round++) {
    await new Promise((resolve) => setTimeout(resolve, 5));
    gc();
    if (until?.()) return;
  }
}

/**
 * Runs collection rounds until an object nothing references is collected, so an assertion that a
 * handle survived follows a collection that ran.
 * @throws {Error} when the object is still alive after the rounds
 */
export async function collectSentinel(): Promise<void> {
  const sentinel = new WeakRef({});
  await collectGarbage(() => sentinel.deref() === undefined);
  if (sentinel.deref() !== undefined) throw new Error("the collector did not run");
}
