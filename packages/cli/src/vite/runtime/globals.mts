// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/**
 * Installs `URL` on `globalThis` if the engine doesn't already provide one.
 * Leaves an existing implementation untouched.
 */

export {};

// oxlint-disable-next-line no-shadow-restricted-names
declare const globalThis: Record<string, unknown>;

if (typeof globalThis.URL === "undefined") {
  globalThis.URL = class URL {
    /**
     * Not spec-compliant: only `.href`, which is all `module-runner`
     * reads back; a `base` argument is ignored.
     */
    href: string;
    constructor(input: string) {
      this.href = input;
    }
  };
}
