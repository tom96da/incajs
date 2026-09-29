#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// A stand-in that reports an app that threw while loading, the way the real
// host does: an error response with a null id, then exit 1. An entry
// argument of `parse` makes it a -32700 reply instead.

const code = process.argv[3] === "parse" ? -32700 : -32000;

process.stdout.write(
  `${JSON.stringify({
    jsonrpc: "2.0",
    id: null,
    error: { code, message: "boom", data: { stack: "Error: boom\n    at somewhere" } },
  })}\n`,
);

setTimeout(() => process.exit(1), 50);
