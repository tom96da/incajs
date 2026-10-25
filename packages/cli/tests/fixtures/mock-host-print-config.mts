#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// A stand-in for `--print-config`: prints its arguments and the `inca.json`
// beside the entry it was given. `MOCK_HOST_STDERR` is written to stderr with
// `{file}` replaced by that `inca.json`'s path and `{other}` by a path of the
// same length that names another file. `MOCK_HOST_EXIT` picks the exit code,
// or a signal name to die from. It reads `{}` for an `inca.json` that is
// missing or not JSON.

import { readFileSync } from "node:fs";
import path from "node:path";

const [flag, entry = ""] = process.argv.slice(2);
const file = path.join(path.dirname(entry), "inca.json");
let config = {};
try {
  config = JSON.parse(readFileSync(file, "utf8"));
} catch {}
process.stdout.write(JSON.stringify({ flag, entry, config }));
const other = file.replace(/json$/, "jsom");
process.stderr.write(
  (process.env.MOCK_HOST_STDERR ?? "").replaceAll("{file}", file).replaceAll("{other}", other),
);

const exit = process.env.MOCK_HOST_EXIT ?? "0";
if (exit.startsWith("SIG")) process.kill(process.pid, exit);
else process.exit(Number(exit));
