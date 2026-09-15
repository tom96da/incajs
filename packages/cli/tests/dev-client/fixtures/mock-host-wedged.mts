#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Like mock-host.mts, but never exits on `shutdown` — a stand-in for an app
// stuck in a loop, so a test can exercise HostClient.stop()'s kill fallback.

import readline from "node:readline";

readline.createInterface({ input: process.stdin }).on("line", (line) => {
  const { id, method }: { id: number; method: string } = JSON.parse(line);
  if (method === "shutdown") {
    process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, result: null })}\n`);
    // Deliberately does not exit.
  }
});
