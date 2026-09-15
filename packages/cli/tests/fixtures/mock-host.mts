#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// A stand-in for `inca-host --dev`: answers `reload`/`shutdown`
// successfully, so `dev()` can be tested without a real engine or window.

import readline from "node:readline";

process.stdout.write(
  `${JSON.stringify({ jsonrpc: "2.0", method: "ready", params: { protocol: 0 } })}\n`,
);
process.stderr.write("mock host: booted\n");

readline.createInterface({ input: process.stdin }).on("line", (line) => {
  const { id, method }: { id: number; method: string } = JSON.parse(line);
  process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, result: null })}\n`);
  if (method === "shutdown") process.exit(0);
});
