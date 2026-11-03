---
description: Where the timers and frame callbacks differ from the Web standards.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Timers

- Their types come with `incajs`.
- A handler must be a function. A string or any other value throws a
  `TypeError`.
- An interval keeps its schedule. After a late run, the next run is the next
  point of the original schedule, and the runs it missed are skipped.
- After 2^31 - 1 timers, frame callbacks and event listeners in one run of the
  app, `setTimeout`, `setInterval`, `requestAnimationFrame` and
  `addEventListener` throw a `RangeError`. A reload of the development window
  starts the count again.
- On macOS, `requestAnimationFrame` callbacks wait while another window fully
  covers the app's window, and run when it is uncovered.
