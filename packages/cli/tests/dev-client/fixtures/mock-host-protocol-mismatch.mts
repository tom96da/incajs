#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// A stand-in that reports a protocol revision this package doesn't
// recognize, so its mismatch handling can be tested without a second host
// binary.

import readline from "node:readline";

process.stdout.write(
  `${JSON.stringify({ jsonrpc: "2.0", method: "ready", params: { protocol: 99 } })}\n`,
);

// Kept open so the process exits only when HostClient kills it.
readline.createInterface({ input: process.stdin });
