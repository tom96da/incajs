// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/** What the QuickJS run needs beyond the generic runner: the `node:` modules of the tests. */

import dedent from "ts-dedent";
import type { Plugin } from "rolldown";

const virtual: Record<string, string> = {
  "node:v8": "export const setFlagsFromString = () => {};",
  // allocates and drops 100 000 cyclic objects; a no-op stub fails createApp and shadowTree
  "node:vm": dedent`
    export const runInNewContext = () => () => {
      let keep = [];
      for (let i = 0; i < 100000; i++) {
        const a = {};
        a.self = a;
        keep.push(a);
      }
      keep = [];
    };
  `,
};

const plugin: Plugin = {
  name: "quickjs-node-modules",
  resolveId: (id) => (id in virtual ? `\0${id}` : null),
  load: (id) => (id.startsWith("\0node:") ? virtual[id.slice(1)] : null),
};

export default {
  plugins: [plugin],
  // a module of vitest creates a TextDecoder
  stubs: [
    "globalThis.TextDecoder = class { decode() { return ''; } };",
    "globalThis.Document = class Document {};",
  ],
  modules: ["@vue/runtime-core/dist/runtime-core.cjs.prod.js"],
};
