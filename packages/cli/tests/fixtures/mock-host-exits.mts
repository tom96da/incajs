#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// A stand-in that quits on its own shortly after loading, the way a real
// host does when its window is closed or it crashes. `MOCK_HOST_EXIT` picks
// the exit code, or a signal name to die from.

process.stdout.write(
  `${JSON.stringify({ jsonrpc: "2.0", method: "ready", params: { protocol: 0 } })}\n`,
);

const exit = process.env.MOCK_HOST_EXIT ?? "0";
setTimeout(() => {
  if (exit.startsWith("SIG")) process.kill(process.pid, exit);
  else process.exit(Number(exit));
}, 50);
