// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/** The handles `keep` holds. */
const alive: object[] = [];

/**
 * Keeps a handle alive until the file ends. QuickJS frees an object as soon as its last strong
 * reference goes, and the node tables hold an unparented handle only weakly.
 * @param handle - the handle to keep
 * @returns `handle`
 */
export function keep<T extends object>(handle: T): T {
  alive.push(handle);
  return handle;
}
