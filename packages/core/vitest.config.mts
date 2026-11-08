// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { defineProject } from "vitest/config";

export default defineProject({ test: { setupFiles: ["./tests/setup.mts"] } });
