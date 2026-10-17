// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { expect } from "vitest";

import type { HostClient } from "../../packages/cli/src/dev-client/hostClient.mts";

const inFlight = new Set<Promise<void>>();
const failures: unknown[] = [];

/**
 * Sends `reload` and keeps the call until the host answers, so teardown can wait for the reply.
 * @param client - the host to reload, or `undefined` before it starts
 * @param timeoutMs - how long the host is given to answer
 */
export function reloadHost(client: HostClient | undefined, timeoutMs: number): void {
  if (!client) return;
  const call = client.call("reload", undefined, timeoutMs).then(
    () => undefined,
    (error: unknown) => {
      failures.push(error);
    },
  );
  inFlight.add(call);
  void call.finally(() => inFlight.delete(call));
}

/** Waits for every `reload` reply, then fails when a call was rejected. Run it before stopping the host. */
export async function settleReloads(): Promise<void> {
  await Promise.all(inFlight);
  expect(failures).toEqual([]);
}
