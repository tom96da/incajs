// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/**
 * Installs `TextDecoder` and `URL` on `globalThis` if the engine doesn't
 * already provide one. Leaves an existing implementation untouched.
 */

if (typeof globalThis.TextDecoder === "undefined") {
  globalThis.TextDecoder = class TextDecoder {
    /**
     * Not a real UTF-8 decode — only `module-runner`'s inline-sourcemap
     * decoding calls it, and this runtime never reaches that path.
     */
    decode(bytes: Iterable<number>): string {
      let out = "";
      for (const byte of bytes) out += String.fromCharCode(byte);
      return out;
    }
  } as any;
}

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
  } as any;
}
