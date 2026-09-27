#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// A stand-in for `inca-host --dev` in HMR mode: answers `reload`/`shutdown`
// like `mock-host.mts`, and echoes any `vite` notification straight back,
// so a test can confirm a payload sent to the app makes it back through
// `HostClient`'s `integrations`.

import readline from "node:readline";

process.stdout.write(
  `${JSON.stringify({ jsonrpc: "2.0", method: "ready", params: { protocol: 0 } })}\n`,
);

readline.createInterface({ input: process.stdin }).on("line", (line) => {
  const message: { id?: number; method: string; params?: unknown } = JSON.parse(line);

  if (message.method === "vite") {
    process.stdout.write(
      `${JSON.stringify({ jsonrpc: "2.0", method: "vite", params: message.params })}\n`,
    );
    return;
  }
  if (message.id === undefined) return;

  process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id: message.id, result: null })}\n`);
  if (message.method === "shutdown") process.exit(0);
});
