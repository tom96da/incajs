---
description: Where the timers differ from the Web standards.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Timers

- The timers and their types come with `incajs`.
- A handler must be a function. A string or any other value throws a
  `TypeError`.
- An interval keeps its schedule. After a late run, the next run is the next
  point of the original schedule, and the runs it missed are skipped.
- After 2^31 - 1 timers in one run of the app, `setTimeout` and `setInterval`
  throw a `RangeError`.
