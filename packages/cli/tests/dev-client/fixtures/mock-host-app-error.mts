#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// A stand-in that reports an app fault right after booting, so HostClient's
// `appError` handling can be tested without a real app that throws.

import readline from "node:readline";

process.stdout.write(
  `${JSON.stringify({ jsonrpc: "2.0", method: "ready", params: { protocol: 0 } })}\n`,
);
process.stdout.write(
  `${JSON.stringify({
    jsonrpc: "2.0",
    method: "appError",
    params: { message: "boom", stack: "Error: boom\n    at somewhere" },
  })}\n`,
);

readline.createInterface({ input: process.stdin }).on("line", (line) => {
  const { id, method }: { id: number; method: string } = JSON.parse(line);
  if (method === "shutdown") {
    process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, result: null })}\n`);
    process.exit(0);
  }
});
