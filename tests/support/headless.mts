// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Runs before every test file (`setupFiles` in `vitest.config.mts`). Removes
// the display variables so every host these tests spawn inherits an
// environment with no display. On Linux gpui then uses its headless platform,
// so the tests need no X server and a stale `DISPLAY` cannot break them.
// macOS ignores both variables.

delete process.env.DISPLAY;
delete process.env.WAYLAND_DISPLAY;
