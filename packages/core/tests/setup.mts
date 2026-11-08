// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The globals the host defines before an app runs.

Object.defineProperty(globalThis, "Document", {
  value: class Document {},
  writable: true,
  configurable: true,
});
