// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Runs one app under the real `inca-host` binary twice, as `inca dev`
// serves it and as a production build, and compares what the app prints about
// its timer, frame, encoding and URL globals. It also checks that
// `incajs/globals` declares the members the host defines and that a script
// using them compiles against the declared types.

import { spawn } from "node:child_process";
import { access, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";

import { dedent } from "ts-dedent";
import ts from "typescript";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";

import { defaultBundler } from "../../packages/cli/src/defaultPlugins.mts";
import { HostClient } from "../../packages/cli/src/dev-client/client.mts";
import { resolveTestHostBin } from "../support/hostBin.mts";
import { reloadHost, settleReloads } from "../support/reloads.mts";
import type { HmrChannel } from "../../packages/cli/src/types.mts";

/** Generous: a real process spawn, a real dev server and a real build sit in the loop. */
const WAIT_TIMEOUT_MS = 30_000;
/** How long the host is given to reload before this test gives up on it. */
const RELOAD_TIMEOUT_MS = 10_000;
/**
 * What the app prints: a hiragana letter decoded from bytes, a resolved URL, a query string and
 * the types of the injected `document` and of `globalThis.document`, then the same two for
 * `window`.
 */
const EXPECTED =
  "[e2e] function function function function function function function あ http://example.com/a%20b?x=1#h a=1+2 object undefined object undefined";

const ENTRY = dedent`
  console.log(
    "[e2e] " +
      typeof setTimeout +
      " " +
      typeof setInterval +
      " " +
      typeof clearTimeout +
      " " +
      typeof requestAnimationFrame +
      " " +
      typeof cancelAnimationFrame +
      " " +
      typeof TextEncoder +
      " " +
      typeof TextDecoder +
      " " +
      new TextDecoder().decode(new Uint8Array([0xe3, 0x81, 0x82])) +
      " " +
      new URL("../a b?x=1#h", "http://example.com/d/e").href +
      " " +
      new URLSearchParams({ a: "1 2" }).toString() +
      " " +
      typeof document +
      " " +
      typeof globalThis.document +
      " " +
      typeof window +
      " " +
      typeof globalThis.window,
  );
`;

/** The events that have a handler property, in the order the host defines them on `window`. */
const HANDLER_EVENTS = [
  "auxclick",
  "beforeinput",
  "blur",
  "change",
  "click",
  "contextmenu",
  "dblclick",
  "focus",
  "gotpointercapture",
  "input",
  "keydown",
  "keypress",
  "keyup",
  "lostpointercapture",
  "mousedown",
  "mouseenter",
  "mouseleave",
  "mousemove",
  "mouseout",
  "mouseover",
  "mouseup",
  "pointerdown",
  "pointerenter",
  "pointerleave",
  "pointermove",
  "pointerout",
  "pointerover",
  "pointerup",
  "scroll",
  "wheel",
];

/** The repository's built CLI runtime, which the development server loads in the host. */
const RUNTIME_DIST = path.resolve(import.meta.dirname, "../../packages/cli/dist/vite-runtime.js");

/** A writable that discards everything and silences the bundler's own logging. */
function discard(): Writable {
  return new Writable({
    write(_chunk, _encoding, callback) {
      callback();
    },
  });
}

/** The first complete line the app printed in `text`. */
function printed(text: string): string | undefined {
  return text
    .split("\n")
    .slice(0, -1)
    .find((line) => line.startsWith("[e2e] "));
}

const scratchRoot = path.join(import.meta.dirname, "tmp", "host-globals");
let appDir: string;
let entry: string;
let channel: HmrChannel | undefined;
let hostClient: HostClient | undefined;

beforeAll(async () => {
  await access(RUNTIME_DIST).catch(() => {
    throw new Error(
      `missing build output: ${RUNTIME_DIST}. run \`pnpm --filter @incajs/cli build\` first`,
    );
  });
  await mkdir(scratchRoot, { recursive: true });
  appDir = await mkdtemp(path.join(scratchRoot, "app-"));
  entry = path.join(appDir, "entry.mts");
  await writeFile(entry, ENTRY);
}, WAIT_TIMEOUT_MS);

afterAll(async () => {
  try {
    await channel?.close();
    await settleReloads();
  } finally {
    await hostClient?.stop();
    await rm(scratchRoot, { recursive: true, force: true });
  }
});

it(
  "prints the same globals in development and in production",
  async () => {
    const prodLine = await runInHost(entry);

    let devText = "";
    channel = await defaultBundler.hmr!({
      entry,
      cwd: appDir,
      stdout: discard(),
      stderr: discard(),
      quiet: true,
      notify: (payload) => hostClient?.notify("vite", payload),
      reload: () => reloadHost(hostClient, RELOAD_TIMEOUT_MS),
      onError: (error) => console.error(error.message),
    });
    hostClient = new HostClient({
      entryFile: channel.entryFile,
      hostBin: resolveTestHostBin(),
      onStderr: (line) => {
        devText += line;
      },
      integrations: { vite: (params) => channel?.dispatch(params) },
    });
    await hostClient.start();
    await vi.waitFor(() => expect(printed(devText)).toBeDefined(), { timeout: WAIT_TIMEOUT_MS });

    expect([printed(devText), prodLine]).toEqual([EXPECTED, EXPECTED]);
  },
  WAIT_TIMEOUT_MS * 3,
);

/** The declaration file of the global types. */
const GLOBALS_DTS = path.resolve(import.meta.dirname, "../../packages/core/dist/globals.d.mts");
/** The globals the host does not define: the build binds them to an import. */
// The host leaves `document` and `window` undefined, so the shape comparison has nothing to read.
const BOUND_BY_IMPORT = new Set(["document", "window"]);
/** What a function or class carries besides its members. */
const FUNCTION_OWN = new Set(["length", "name", "prototype"]);

interface Shape {
  type: string;
  members: string[];
  prototypeMembers: string[] | null;
}

/** The names of the properties of `type` that a script can list, without symbol keys. */
function memberNames(checker: ts.TypeChecker, type: ts.Type, skip = new Set<string>()): string[] {
  return checker
    .getPropertiesOfType(type)
    .map((symbol) => symbol.getName())
    .filter((name) => !name.startsWith("__@") && !skip.has(name))
    .sort();
}

/** The shape of every global that `globals.d.mts` declares, read with the compiler. */
function declaredShapes(): Record<string, Shape> {
  const program = ts.createProgram([GLOBALS_DTS], {
    lib: ["lib.es2025.d.ts"],
    types: [],
    noEmit: true,
  });
  const checker = program.getTypeChecker();
  const source = program.getSourceFile(GLOBALS_DTS)!;
  const shapes: Record<string, Shape> = {};
  const visit = (node: ts.Node) => {
    if (ts.isFunctionDeclaration(node) && node.name) {
      shapes[node.name.text] = { type: "function", members: [], prototypeMembers: null };
    }
    if (ts.isVariableStatement(node)) {
      for (const declaration of node.declarationList.declarations) {
        const name = (declaration.name as ts.Identifier).text;
        if (BOUND_BY_IMPORT.has(name)) continue;
        const type = checker.getTypeAtLocation(declaration);
        const prototype = type.getProperty("prototype");
        shapes[name] = prototype
          ? {
              type: "function",
              members: memberNames(checker, type, FUNCTION_OWN),
              prototypeMembers: memberNames(checker, checker.getTypeOfSymbol(prototype)),
            }
          : { type: "object", members: memberNames(checker, type), prototypeMembers: null };
      }
    }
    if (ts.isModuleBlock(node) || ts.isModuleDeclaration(node)) {
      ts.forEachChild(node, visit);
    }
  };
  ts.forEachChild(source, visit);
  return shapes;
}

/** Builds `entryPath`, runs the bundle in the host and returns the `[e2e] ` line it printed. */
async function runInHost(entryPath: string): Promise<string> {
  const outDir = path.join(path.dirname(entryPath), "dist");
  await defaultBundler.build({
    entry: entryPath,
    outDir,
    quiet: true,
    stdout: discard(),
    stderr: discard(),
  });
  const child = spawn(resolveTestHostBin(), [path.join(outDir, "bundle.js")], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  let text = "";
  child.stdout.on("data", (chunk: Buffer) => (text += chunk.toString()));
  child.stderr.on("data", (chunk: Buffer) => (text += chunk.toString()));
  try {
    await vi.waitFor(() => expect(printed(text)).toBeDefined(), { timeout: WAIT_TIMEOUT_MS });
  } finally {
    child.kill();
  }
  return printed(text)!;
}

it(
  "declares the members the host defines on every declared global",
  async () => {
    const declared = declaredShapes();
    const shapeEntry = path.join(appDir, "shape.mts");
    await writeFile(
      shapeEntry,
      dedent`
        const chain = (value) => {
          const list = [];
          for (let o = value; o && o !== Object.prototype; o = Object.getPrototypeOf(o)) {
            list.push(o);
          }
          return list;
        };
        const own = (values, skip) =>
          [...new Set(values.flatMap((o) => Object.getOwnPropertyNames(o)))]
            .filter((n) => !skip.includes(n))
            .sort();
        const shapes = {};
        for (const name of ${JSON.stringify(Object.keys(declared))}) {
          const value = globalThis[name];
          const isFunction = typeof value === "function";
          shapes[name] = {
            type: typeof value,
            members: isFunction
              ? own([value], ["length", "name", "prototype"])
              : own(chain(value), ["constructor"]),
            prototypeMembers:
              isFunction && value.prototype ? own(chain(value.prototype), ["constructor"]) : null,
          };
        }
        console.log("[e2e] " + JSON.stringify(shapes));
      `,
    );
    const line = await runInHost(shapeEntry);
    const hostShapes = JSON.parse(line.slice("[e2e] ".length)) as Record<string, Shape>;
    expect(hostShapes).toEqual(declared);
  },
  WAIT_TIMEOUT_MS * 2,
);

it(
  "prints a node as its interface and its own properties",
  async () => {
    const printEntry = path.join(appDir, "print.mts");
    await writeFile(
      printEntry,
      dedent`
        const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
        console.log(
          "[e2e]",
          document.createElement("div"),
          document.createTextNode("x"),
          document.createElement("input"),
          document.createElement("template"),
          svg,
          window,
        );
      `,
    );
    const handlers = HANDLER_EVENTS.map((event) => `on${event}: [Getter/Setter]`).join(", ");

    expect(await runInHost(printEntry)).toBe(
      "[e2e] HTMLElement {} Text {} HTMLElement { value: [Getter/Setter] } " +
        `HTMLElement { content: [Getter] } Element {} EventTarget { ${handlers} }`,
    );
  },
  WAIT_TIMEOUT_MS * 2,
);

/** Each row is the outcome in words, the value to clone and what the script prints for it. */
it.each([
  ["throws DataCloneError for an element", 'document.createElement("div")', "DataCloneError"],
  ["throws DataCloneError for document", "document", "DataCloneError"],
  ["throws DataCloneError for window", "window", "DataCloneError"],
  ["throws DataCloneError for an event", 'new Event("x")', "DataCloneError"],
  [
    "gives {} for an object that only inherits a node prototype",
    "Object.create(HTMLElement.prototype)",
    "{}",
  ],
])(
  "structuredClone %s",
  async (_name, value, expected) => {
    const cloneEntry = path.join(appDir, "clone.mts");
    await writeFile(
      cloneEntry,
      dedent`
        let outcome;
        try {
          outcome = JSON.stringify(structuredClone(${value}));
        } catch (error) {
          outcome = error.name;
        }
        console.log("[e2e] " + outcome);
      `,
    );

    expect(await runInHost(cloneEntry)).toBe(`[e2e] ${expected}`);
  },
  WAIT_TIMEOUT_MS * 2,
);

it("compiles a script that uses the declared globals with no other types", async () => {
  const useEntry = path.join(appDir, "use.mts");
  await writeFile(
    useEntry,
    dedent`
      requestAnimationFrame(() => {});
      performance.now();
      new URL("x");
      const copy: number[] = structuredClone([1], { transfer: [] });
      document.createElement("div");
      window.addEventListener("keydown", () => {});
      document.dispatchEvent(new Event("x"));
      const detail: number = new CustomEvent("x", { detail: 1 }).detail;
      const none: 0 = Event.NONE;
      // @ts-expect-error CustomEvent has no static of its own
      CustomEvent.NONE;
      // @ts-expect-error eventPhase has the four values of the standard
      const phase: 0 | 2 = new Event("x").eventPhase;
    `,
  );
  const options: ts.CompilerOptions = {
    lib: ["lib.es2025.d.ts"],
    types: ["incajs/globals"],
    module: ts.ModuleKind.Preserve,
    noEmit: true,
    strict: true,
  };
  // A `types` name resolves from the current directory, which differs between runners.
  const host = ts.createCompilerHost(options);
  host.getCurrentDirectory = () => appDir;
  const program = ts.createProgram([useEntry], options, host);
  expect(
    ts
      .getPreEmitDiagnostics(program)
      .map((d) => ts.flattenDiagnosticMessageText(d.messageText, "\n")),
  ).toEqual([]);
});

/** The built declarations of the package. */
const DIST = path.resolve(import.meta.dirname, "../../packages/core/dist");

/** `dist/index.d.ts` only re-exports from the chunks it imports, so the text is the entry with them. */
async function declarations(): Promise<string> {
  const entry = await readFile(path.join(DIST, "index.d.ts"), "utf8");
  const chunks = [...entry.matchAll(/from "(\.\/chunks\/[^"]+)\.js"/g)].map((match) =>
    readFile(path.join(DIST, `${match[1]!}.d.ts`), "utf8"),
  );
  return [entry, ...(await Promise.all(chunks))].join("\n");
}

describe("the declarations of incajs", () => {
  it("have no readonly kind", async () => {
    expect(await declarations()).not.toContain("readonly kind");
  });

  it("have no __node_id", async () => {
    expect(await declarations()).not.toContain("__node_id");
  });

  it("declare readonly eventId once", async () => {
    expect((await declarations()).match(/readonly eventId/g)).toHaveLength(1);
  });
});
