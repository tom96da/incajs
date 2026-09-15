#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// A stand-in host that sends an unrecognized notification method (as a
// future bundler integration would), so integration routing can be tested.

import readline from "node:readline";

process.stdout.write(
  `${JSON.stringify({ jsonrpc: "2.0", method: "ready", params: { protocol: 0 } })}\n`,
);
process.stdout.write(
  `${JSON.stringify({ jsonrpc: "2.0", method: "someIntegration", params: { hello: "world" } })}\n`,
);

readline.createInterface({ input: process.stdin }).on("line", (line) => {
  const { id, method }: { id: number; method: string } = JSON.parse(line);
  if (method === "shutdown") {
    process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, result: null })}\n`);
    process.exit(0);
  }
});
