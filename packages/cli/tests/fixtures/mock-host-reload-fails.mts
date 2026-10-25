#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// A stand-in whose `reload` always answers with a JSON-RPC error, the way
// a bundle that throws while being evaluated does, so dev()'s failed-reload
// display can be tested without a real broken bundle.

import "./answerPrintConfig.mts";
import readline from "node:readline";

process.stdout.write(
  `${JSON.stringify({ jsonrpc: "2.0", method: "ready", params: { protocol: 0 } })}\n`,
);

readline.createInterface({ input: process.stdin }).on("line", (line) => {
  const { id, method }: { id: number; method: string } = JSON.parse(line);
  if (method === "reload") {
    process.stdout.write(
      `${JSON.stringify({
        jsonrpc: "2.0",
        id,
        error: { code: -32000, message: "boom", data: { stack: "Error: boom\n    at somewhere" } },
      })}\n`,
    );
    return;
  }
  process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, result: null })}\n`);
  if (method === "shutdown") process.exit(0);
});
