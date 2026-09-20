#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// A stand-in that quits on its own shortly after loading, the way a real
// host does when its window is closed or it crashes.

process.stdout.write(
  `${JSON.stringify({ jsonrpc: "2.0", method: "ready", params: { protocol: 0 } })}\n`,
);

setTimeout(() => process.exit(0), 50);
