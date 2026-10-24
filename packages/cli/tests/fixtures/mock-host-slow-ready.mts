#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// A stand-in whose `ready` notification lands late, so a build that
// finishes before it can be tested without a real slow-starting host.
// With `MOCK_HOST_READY_GATE` set to a file path, `ready` lands once that file exists.

import { existsSync } from "node:fs";
import readline from "node:readline";

let reloadCount = 0;

const ready = (): void => {
  process.stdout.write(
    `${JSON.stringify({ jsonrpc: "2.0", method: "ready", params: { protocol: 0 } })}\n`,
  );
};
const gate = process.env.MOCK_HOST_READY_GATE;
if (gate) {
  const poll = setInterval(() => {
    if (!existsSync(gate)) return;
    clearInterval(poll);
    ready();
  }, 10);
} else {
  setTimeout(ready, 200);
}

readline.createInterface({ input: process.stdin }).on("line", (line) => {
  const { id, method }: { id: number; method: string } = JSON.parse(line);
  if (method === "reload") {
    reloadCount += 1;
    process.stderr.write(`reload #${reloadCount}\n`);
  }
  process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, result: null })}\n`);
  if (method === "shutdown") process.exit(0);
});
