#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// A stand-in for `inca-host --dev` that reports the color variables it was
// started with on stderr, as `env NO_COLOR=<json> FORCE_COLOR=<json>` (`null` when unset).

import readline from "node:readline";

process.stdout.write(
  `${JSON.stringify({ jsonrpc: "2.0", method: "ready", params: { protocol: 0 } })}\n`,
);
process.stderr.write(
  `env NO_COLOR=${JSON.stringify(process.env.NO_COLOR ?? null)} ` +
    `FORCE_COLOR=${JSON.stringify(process.env.FORCE_COLOR ?? null)}\n`,
);

readline.createInterface({ input: process.stdin }).on("line", (line) => {
  const { id, method }: { id: number; method: string } = JSON.parse(line);
  process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, result: null })}\n`);
  if (method === "shutdown") process.exit(0);
});
