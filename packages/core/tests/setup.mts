// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The globals the host defines before an app runs.

// A test file imports this module for `nativeStub`, so the QuickJS run evaluates it a second time
// after its own stubs.
if (!("Document" in globalThis)) {
  Object.defineProperty(globalThis, "Document", {
    value: class Document {},
    writable: true,
    configurable: true,
  });
}

/** The host object core calls. */
type Native = typeof globalThis.__inca_native__;

/**
 * A stand-in for `__inca_native__` with the calls core makes while it builds `document`.
 * @param extra - the calls and values a test adds or replaces
 * @returns the stand-in, in which `documentNodeId()` gives 1
 */
export function nativeStub(extra: Partial<Native> = {}): Native {
  return { documentNodeId: () => 1, ...extra } as Native;
}

globalThis.__inca_native__ ??= nativeStub();
