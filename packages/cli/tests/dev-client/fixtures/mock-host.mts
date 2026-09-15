#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// A stand-in for `gpjs-ui-host --dev`, driven by dev-client's own tests
// instead of a real compiled binary: it speaks the same newline-delimited
// JSON-RPC channel the real host does, just without a real engine or
// window behind it.

import readline from "node:readline";

process.stdout.write(
  "not json — noise from some dependency, as the real host's own stdout can carry\n",
);
process.stdout.write(
  `${JSON.stringify({ jsonrpc: "2.0", method: "ready", params: { protocol: 0 } })}\n`,
);
process.stderr.write("mock host: booted\n");

readline.createInterface({ input: process.stdin }).on("line", (line) => {
  const { id, method }: { id: number; method: string } = JSON.parse(line);
  if (method === "reload" || method === "shutdown") {
    process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, result: null })}\n`);
    if (method === "shutdown") process.exit(0);
    return;
  }
  process.stdout.write(
    `${JSON.stringify({ jsonrpc: "2.0", id, error: { code: -32601, message: "unknown method" } })}\n`,
  );
});
