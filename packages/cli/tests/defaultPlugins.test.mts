// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, mkdtemp, readdir, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { Writable } from "node:stream";
import { pathToFileURL } from "node:url";

import dedent from "ts-dedent";
import { afterAll, afterEach, beforeAll, expect, it } from "vitest";

import { defaultBundler } from "../src/defaultPlugins.mts";

const scratchRoot = path.join(import.meta.dirname, "tmp", "default-bundler");

beforeAll(() => mkdir(scratchRoot, { recursive: true }));
afterAll(() => rm(scratchRoot, { recursive: true, force: true }));

/**
 * Builds an app whose template is `template` through `defaultBundler`, with `incajs/vue/runtime`
 * stubbed as the Vue core plus `runtimeExtra`. The entry starts with `imports`.
 * @returns the text of the bundle
 */
async function bundleOf(template: string, runtimeExtra: string, imports = ""): Promise<string> {
  const appDir = await mkdtemp(path.join(scratchRoot, "app-"));
  const stub = path.join(appDir, "node_modules/incajs");
  await mkdir(stub, { recursive: true });
  await writeFile(
    path.join(stub, "package.json"),
    JSON.stringify({
      name: "incajs",
      type: "module",
      exports: { ".": "./index.mjs", "./vue/runtime": "./runtime.mjs" },
    }),
  );
  await writeFile(
    path.join(stub, "index.mjs"),
    `export const document = { title: "DOCUMENT_STUB_MARKER" };\n`,
  );
  await writeFile(
    path.join(stub, "runtime.mjs"),
    `export * from "@vue/runtime-core";\n${runtimeExtra}`,
  );
  await writeFile(path.join(appDir, "App.vue"), `<template>${template}</template>\n`);
  const entry = path.join(appDir, "entry.mts");
  await writeFile(entry, `${imports}import App from "./App.vue";\nexport default App;\n`);
  const sink = new Writable({ write: (_chunk, _encoding, callback) => callback() });

  const output = await defaultBundler.build({
    entry,
    outDir: path.join(appDir, "dist"),
    stdout: sink,
    stderr: sink,
    quiet: true,
  });

  return readFile(output.entryFile, "utf8");
}

it("binds document in a dependency that sits outside the app", async () => {
  const outside = await realpath(await mkdtemp(path.join(tmpdir(), "inca-dependency-")));
  const dependency = path.join(outside, "dependency.mjs");
  await writeFile(dependency, "globalThis.title = document.title;\n");

  const bundle = await bundleOf("<i>x</i>", "", `import ${JSON.stringify(dependency)};\n`);
  await rm(outside, { recursive: true, force: true });

  expect(bundle).toContain("DOCUMENT_STUB_MARKER");
}, 20000);

it("builds templates against incajs/vue/runtime", async () => {
  const bundle = await bundleOf(
    `<div @click.stop="1">x</div>`,
    `export const withModifiers = (fn) => { "DEFAULT_RUNTIME_MARKER"; return fn; };\n`,
  );

  expect(bundle).toContain("DEFAULT_RUNTIME_MARKER");
}, 20000);

it("compiles many static elements without createStaticVNode", async () => {
  const bundle = await bundleOf(
    "<i>x</i>".repeat(25),
    `export const createStaticVNode = (...args) => { "STATIC_MARKER"; return args; };\n`,
  );

  expect(bundle).not.toContain("STATIC_MARKER");
}, 20000);

it("checks the peers that incajs lists for vue", async () => {
  const appDir = await mkdtemp(path.join(scratchRoot, "app-"));
  const stub = path.join(appDir, "node_modules/incajs");
  await mkdir(stub, { recursive: true });
  await writeFile(
    path.join(stub, "package.json"),
    JSON.stringify({
      name: "incajs",
      type: "module",
      exports: { "./vue/runtime": "./runtime.mjs" },
      incajs: { frontends: { vue: { peers: ["@vue/runtime-dom"] } } },
    }),
  );
  await writeFile(path.join(stub, "runtime.mjs"), `export * from "@vue/runtime-core";\n`);
  await writeFile(
    path.join(appDir, "App.vue"),
    dedent`
      <script setup>
      import "@vue/runtime-dom";
      </script>
      <template><div /></template>
    `,
  );
  const entry = path.join(appDir, "entry.mts");
  await writeFile(entry, `import App from "./App.vue";\nexport default App;\n`);
  const sink = new Writable({ write: (_chunk, _encoding, callback) => callback() });

  await expect(
    defaultBundler.build({
      entry,
      outDir: path.join(appDir, "dist"),
      stdout: sink,
      stderr: sink,
      quiet: true,
    }),
  ).rejects.toThrow("ERR_INCA_PEER_NOT_FOUND");
}, 20000);

type StyleCall = [key: string, css: string];

let styleRoot: string;
beforeAll(async () => {
  // Vite hands out real paths, and the temporary directory of macOS is a symlink.
  styleRoot = await realpath(await mkdtemp(path.join(tmpdir(), "inca-style-")));
});
afterAll(() => rm(styleRoot, { recursive: true, force: true }));
afterEach(() => Reflect.deleteProperty(globalThis, "__styleCalls"));

/**
 * Builds an app of `files` (the entry imports `./App.vue`) through `defaultBundler`, in a
 * directory outside every named package, with `incajs/style` stubbed to record its calls.
 * @returns the files the build wrote and the calls of `updateStyle` that running the bundle makes
 */
async function stylesOf(
  files: Record<string, string>,
): Promise<{ written: string[]; calls: StyleCall[] }> {
  const appDir = await mkdtemp(path.join(styleRoot, "app-"));
  const stub = path.join(appDir, "node_modules/incajs");
  await mkdir(stub, { recursive: true });
  await writeFile(
    path.join(stub, "package.json"),
    JSON.stringify({
      name: "incajs",
      type: "module",
      exports: { "./vue/runtime": "./runtime.mjs", "./style": "./style.mjs" },
    }),
  );
  await writeFile(
    path.join(stub, "runtime.mjs"),
    "export const openBlock = () => {};\nexport const createElementBlock = () => {};\n",
  );
  await writeFile(
    path.join(stub, "style.mjs"),
    dedent`
      export function updateStyle(key, css) {
        globalThis.__styleCalls.push([key, css]);
      }
    `,
  );
  await writeFile(path.join(appDir, "package.json"), "{}");
  for (const [name, text] of Object.entries(files)) {
    await writeFile(path.join(appDir, name), text);
  }
  const entry = path.join(appDir, "entry.mts");
  await writeFile(
    entry,
    dedent`
      import App from "./App.vue";
      export default App;
    `,
  );
  const sink = new Writable({ write: (_chunk, _encoding, callback) => callback() });
  const outDir = path.join(appDir, "dist");

  const output = await defaultBundler.build({
    entry,
    outDir,
    stdout: sink,
    stderr: sink,
    quiet: true,
  });

  const calls: StyleCall[] = [];
  Object.assign(globalThis, { __styleCalls: calls });
  await import(/* @vite-ignore */ pathToFileURL(output.entryFile).href);
  return { written: await readdir(outDir), calls };
}

const TEMPLATE = '<template><div class="a">x</div></template>';

/** An `App.vue` that imports the stylesheets `imports` for their side effect. */
function importing(...imports: string[]): string {
  return dedent`
    <script setup>
    ${imports.map((file) => `import "${file}";`).join("\n")}
    </script>
    ${TEMPLATE}
  `;
}

it("applies the CSS of a scoped block under a key relative to the app", async () => {
  const { calls } = await stylesOf({
    "App.vue": dedent`
      ${TEMPLATE}
      <style scoped>
      .a { width: 10px; }
      </style>
    `,
  });

  expect(calls).toHaveLength(1);
  expect(calls[0]![0]).toMatch(/^App\.vue\?vue&type=style&index=0&scoped=[0-9a-f]+&lang\.css$/);
  expect(calls[0]![1]).toContain(".a[data-v-");
});

it("applies a stylesheet import with its CSS inlined", async () => {
  const { calls } = await stylesOf({
    "App.vue": importing("./a.css"),
    "a.css": dedent`
      @import "./b.css";
      .a { width: 10px; }
    `,
    "b.css": ".b { height: 5px; }",
  });

  expect(calls.map(([key]) => key)).toEqual(["a.css"]);
  expect(calls[0]![1]).toContain(".b { height: 5px; }");
});

it("applies a module block with hashed class names", async () => {
  const { calls } = await stylesOf({
    "App.vue": dedent`
      ${TEMPLATE}
      <style module>
      .a { width: 10px; }
      </style>
    `,
  });

  expect(calls).toHaveLength(1);
  expect(calls[0]![1]).toMatch(/\._?a_[\w-]+ \{/);
});

it("applies a CSS module whose class map nothing uses", async () => {
  const { calls } = await stylesOf({
    "App.vue": importing("./a.module.css"),
    "a.module.css": ".a { width: 10px; }",
  });

  expect(calls.map(([key]) => key)).toEqual(["a.module.css"]);
});

it("builds an app whose package has no name without a stylesheet file", async () => {
  const { written } = await stylesOf({
    "App.vue": dedent`
      ${TEMPLATE}
      <style>
      .a { width: 10px; }
      </style>
    `,
  });

  expect(written).toEqual(["bundle.js"]);
});

it("applies the CSS of an import without minifying or rewriting it", async () => {
  const css = dedent`
    /* note */
    .a::before { content: "q"; }
    .a { width: 10px; }
  `;
  const { calls } = await stylesOf({ "App.vue": importing("./a.css"), "a.css": css });

  expect(calls[0]![1]).toBe(css);
});
