#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// A stand-in for `inca-host --dev` that appends `start <entryFile>`, `reload` and
// `stop <entryFile>` to the file named by `MOCK_HOST_LOG`, so a test sees the order of the hosts.
// With `MOCK_HOST_RELOAD_GATE` set, a reload answers once the file `<MOCK_HOST_LOG>.release`
// exists and logs `reloaded` first. A `vite` notification is echoed back. Its process id goes to
// `<MOCK_HOST_LOG>.pid`.

import "./answerPrintConfig.mts";
import { appendFileSync, existsSync, writeFileSync } from "node:fs";
import readline from "node:readline";

const note = (line: string): void => appendFileSync(process.env.MOCK_HOST_LOG!, `${line}\n`);

writeFileSync(`${process.env.MOCK_HOST_LOG}.pid`, String(process.pid));
note(`start ${process.argv[3]}`);
process.stdout.write(
  `${JSON.stringify({ jsonrpc: "2.0", method: "ready", params: { protocol: 0 } })}\n`,
);

readline.createInterface({ input: process.stdin }).on("line", (line) => {
  const { id, method, params }: { id?: number; method: string; params?: unknown } =
    JSON.parse(line);
  const reply = (): void => {
    if (id !== undefined) {
      process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, result: null })}\n`);
    }
  };
  if (method === "vite") {
    process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", method: "vite", params })}\n`);
    return;
  }
  if (method === "reload") note("reload");
  if (method === "reload" && process.env.MOCK_HOST_RELOAD_GATE) {
    const poll = setInterval(() => {
      if (!existsSync(`${process.env.MOCK_HOST_LOG}.release`)) return;
      clearInterval(poll);
      note("reloaded");
      reply();
    }, 10);
    return;
  }
  reply();
  if (method === "shutdown") {
    note(`stop ${process.argv[3]}`);
    process.exit(0);
  }
});
