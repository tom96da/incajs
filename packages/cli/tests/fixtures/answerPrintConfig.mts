// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Imported for its side effect: a mock host that `dev()` starts answers `--print-config`, which
// the CLI runs before each host start. It writes `{}` and exits 0. With `MOCK_HOST_CONFIG_FAULT`
// set it writes that to stderr, `{file}` replaced by the `inca.json` beside the entry, and exits 1.

import path from "node:path";

if (process.argv[2] === "--print-config") {
  const fault = process.env.MOCK_HOST_CONFIG_FAULT;
  if (fault) {
    const file = path.join(path.dirname(process.argv[3] ?? ""), "inca.json");
    process.stderr.write(fault.replaceAll("{file}", file));
    process.exit(1);
  }
  process.stdout.write("{}\n");
  process.exit(0);
}
