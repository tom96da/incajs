#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Runs `build-ui-fixtures.mts` through Vite's module runner, which compiles
// the TypeScript the CLI sources use.

import { runnerImport } from "vite";

await runnerImport(new URL("./build-ui-fixtures.mts", import.meta.url).pathname);
