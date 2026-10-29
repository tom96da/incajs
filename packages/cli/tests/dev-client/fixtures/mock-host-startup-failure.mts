#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// A stand-in that reports an app that threw while loading, the way the real
// host does: an error response with a null id, then exit 1. An entry
// argument of `parse` makes it a -32700 reply instead. An entry argument of
// `panic` sends a -32603 reply and then a -32700 one.

import "../../fixtures/answerPrintConfig.mts";

const mode = process.argv[3];
const codes = mode === "panic" ? [-32603, -32700] : [mode === "parse" ? -32700 : -32000];

for (const code of codes) {
  process.stdout.write(
    `${JSON.stringify({
      jsonrpc: "2.0",
      id: null,
      error: { code, message: "boom", data: { stack: "Error: boom\n    at somewhere" } },
    })}\n`,
  );
}

setTimeout(() => process.exit(1), 50);
