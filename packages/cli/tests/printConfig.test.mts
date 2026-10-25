// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

import { afterAll, beforeAll, expect, it, onTestFinished, vi } from "vitest";

vi.mock("../src/dev-client/index.mts", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../src/dev-client/index.mts")>();
  return { ...actual, resolveHostBin: vi.fn<typeof actual.resolveHostBin>(actual.resolveHostBin) };
});

import { resolveHostBin } from "../src/dev-client/index.mts";
import { IncaError } from "../src/error.mts";
import { checkBuiltConfig, checkConfig, printConfig } from "../src/printConfig.mts";
import { scratchConfigApp } from "./scratchConfigApp.mts";
import { makeSink, makeUnspawnableHost } from "./support/dev.mts";

const mockedResolveHostBin = vi.mocked(resolveHostBin);

const hostBin = path.join(import.meta.dirname, "fixtures/mock-host-print-config.mts");

const scratch = scratchConfigApp("print-config");

beforeAll(scratch.setUp);
afterAll(scratch.tearDown);

/** Makes the mock host exit with `exit` after writing `stderr`, for one test. */
function hostAnswers(exit: string, stderr = ""): void {
  vi.stubEnv("MOCK_HOST_EXIT", exit);
  vi.stubEnv("MOCK_HOST_STDERR", stderr);
  onTestFinished(() => void vi.unstubAllEnvs());
}

/** Runs `printConfig` for the app at `cwd` against the mock host. */
async function run(cwd: string) {
  const stdout = makeSink();
  const stderr = makeSink();
  const code = await printConfig({ cwd, hostBin, stdout: stdout.stream, stderr: stderr.stream });
  return { code, stdout: stdout.text(), stderr: stderr.text() };
}

it("starts the host with --print-config on the entry under node_modules/.inca/config", async () => {
  const app = await scratch.makeApp({});

  const { flag, entry } = JSON.parse((await run(app)).stdout);

  expect(flag).toBe("--print-config");
  expect(entry).toBe(path.join(app, "node_modules/.inca/config/bundle.js"));
  expect(await readFile(entry, "utf8")).toBe("");
});

it("hands the host the config the app resolves to", async () => {
  const app = await scratch.makeApp({ name: "scratch-app" });
  await writeFile(path.join(app, "inca.config.ts"), "export default { window: { width: 640 } };\n");

  const { config } = JSON.parse((await run(app)).stdout);

  expect(config).toMatchObject({ name: "scratch-app", window: { width: 640 } });
});

it("hands the host an app that declares nothing with dev.stackPaths and root", async () => {
  const app = await scratch.makeApp({});
  const dir = path.join(app, "node_modules/.inca/config");
  await mkdir(dir, { recursive: true });
  await writeFile(path.join(dir, "inca.json"), JSON.stringify({ name: "earlier" }));

  const { config } = JSON.parse((await run(app)).stdout);

  expect(config).toEqual({ dev: { stackPaths: "relative" }, root: "../../.." });
});

it.each([
  ["another file", 'failed to read {other}: ["window","width"]: x\n'],
  ["no path", "failed to read {file}: Is a directory (os error 21)\n"],
  ["whole file", "failed to read {file}: []: expected ident at line 1 column 2\n"],
])("writes a host report on no setting (%s) to stderr as it is", async (_, report) => {
  hostAnswers("1", report);
  const app = await scratch.makeApp({});

  const { code, stderr } = await run(app);

  const file = path.join(app, "node_modules/.inca/config/inca.json");
  expect(code).toBe(1);
  const other = file.replace(/json$/, "jsom");
  expect(stderr).toBe(report.replaceAll("{file}", file).replaceAll("{other}", other));
});

it.each([
  ["3", 3],
  ["SIGTERM", 1],
])("returns %s from the host as exit code %i", async (exit, code) => {
  hostAnswers(exit);
  const app = await scratch.makeApp({});

  expect((await run(app)).code).toBe(code);
});

it("rejects a host binary that is not a file", async () => {
  const app = await scratch.makeApp({});

  await expect(printConfig({ cwd: app, hostBin: path.join(app, "missing-host") })).rejects.toThrow(
    expect.objectContaining({ code: "ERR_INCA_HOST_BIN_NOT_FOUND" }),
  );
});

it("rejects with the spawn error of a host that cannot be started", async () => {
  const app = await scratch.makeApp({});

  await expect(printConfig({ cwd: app, hostBin: await makeUnspawnableHost(app) })).rejects.toThrow(
    expect.objectContaining({ code: "EACCES" }),
  );
});

/** Makes an app whose `inca.config.ts` declares `window`. */
async function appWithWindow(window: object): Promise<string> {
  const app = await scratch.makeApp({});
  await writeFile(
    path.join(app, "inca.config.ts"),
    `export default { window: ${JSON.stringify(window)} };\n`,
  );
  return app;
}

it("names the window setting, its value and the kind it must have", async () => {
  hostAnswers("1", 'failed to read {file}: ["window","minWidth"]: invalid type\n');
  const app = await appWithWindow({ minWidth: "x" });

  await expect(run(app)).rejects.toThrow(
    expect.objectContaining({
      code: "ERR_INCA_CONFIG_INVALID",
      message: 'window.minWidth in inca.config.ts is "x". it must be a number',
    }),
  );
});

it("names productName for the name the host cannot use", async () => {
  hostAnswers("1", 'failed to read {file}: ["name"]: invalid type\n');
  const app = await scratch.makeApp({ name: "scratch-app" });

  await expect(run(app)).rejects.toThrow(
    expect.objectContaining({
      message: expect.stringMatching(/^productName in inca\.config\.ts is /),
    }),
  );
});

it("names dev.stackPaths for a value the host cannot use", async () => {
  hostAnswers("1", 'failed to read {file}: ["dev","stackPaths"]: unknown variant\n');
  const app = await scratch.makeApp({});
  await writeFile(
    path.join(app, "inca.config.ts"),
    'export default { dev: { stackPaths: "rel" } };\n',
  );

  await expect(run(app)).rejects.toThrow(
    expect.objectContaining({
      message: 'dev.stackPaths in inca.config.ts is "rel". the host can\'t use it',
    }),
  );
});

it("says the host can't use a value that already has its setting's kind", async () => {
  hostAnswers("1", 'failed to read {file}: ["window"]: trailing characters at line 1 column 29\n');
  const app = await appWithWindow([1, 2, "t", true, 5, 6, 7]);

  await expect(run(app)).rejects.toThrow(
    expect.objectContaining({
      message: 'window in inca.config.ts is [1,2,"t",true,5,6,7]. the host can\'t use it',
    }),
  );
});

it("says a number for a text setting must be a string", async () => {
  hostAnswers("1", 'failed to read {file}: ["window","title"]: invalid type\n');
  const app = await appWithWindow({ title: 5 });

  await expect(run(app)).rejects.toThrow(
    expect.objectContaining({
      message: "window.title in inca.config.ts is 5. it must be a string",
    }),
  );
});

/** Makes an app directory that holds an entry, and `inca.json` when `config` is given. */
async function appWithEntry(config?: object): Promise<{ app: string; entry: string }> {
  const app = await scratch.makeApp({});
  const entry = path.join(app, "bundle.js");
  await writeFile(entry, "");
  if (config) await writeFile(path.join(app, "inca.json"), JSON.stringify(config));
  return { app, entry };
}

const WIDTH_FAULT =
  'failed to read {file}: ["window","width"]: ' +
  "invalid value: -100, expected a finite number above zero\n";

it("checkConfig passes an entry the host accepts", async () => {
  const { entry } = await appWithEntry({});

  expect(() => checkConfig(hostBin, entry)).not.toThrow();
});

it("checkConfig throws the setting the host cannot use", async () => {
  hostAnswers("1", WIDTH_FAULT);
  const { entry } = await appWithEntry({ window: { width: -100 } });

  expect(() => checkConfig(hostBin, entry)).toThrow(
    expect.objectContaining({
      code: "ERR_INCA_CONFIG_INVALID",
      message: "window.width in inca.config.ts is -100. the host can't use it",
    }),
  );
});

it("checkConfig reads an empty config when no inca.json is beside the entry", async () => {
  const { entry } = await appWithEntry();

  expect(() => checkConfig(hostBin, entry)).not.toThrow();
});

it("checkConfig reports a corrupt inca.json as the host's report", async () => {
  hostAnswers("1", "failed to read {file}: []: expected ident at line 1 column 2\n");
  const { app, entry } = await appWithEntry();
  const file = path.join(app, "inca.json");
  await writeFile(file, "not json");

  expect(() => checkConfig(hostBin, entry)).toThrow(
    expect.objectContaining({
      code: "ERR_INCA_CONFIG_INVALID",
      message:
        `the host can't read ${file}. ` +
        `it reported "failed to read ${file}: []: expected ident at line 1 column 2"`,
    }),
  );
});

it.each([
  ["not_found", () => vi.stubEnv("INCA_HOST_BIN", "/nonexistent/inca-host")],
  [
    "unresolved",
    () =>
      mockedResolveHostBin.mockImplementationOnce(() => {
        throw new IncaError("ERR_INCA_HOST_BIN_UNRESOLVED", "no host binary found");
      }),
  ],
])("lets checkBuiltConfig skip the check without a host binary (%s)", async (_, arrange) => {
  arrange();
  onTestFinished(() => void vi.unstubAllEnvs());
  const { entry } = await appWithEntry({});
  const stderr = makeSink();

  checkBuiltConfig(entry, stderr.stream);

  expect(stderr.text()).toContain("no host binary found, so the config was not checked");
});

it("checkBuiltConfig checks with the host it finds", async () => {
  vi.stubEnv("INCA_HOST_BIN", hostBin);
  hostAnswers("1", WIDTH_FAULT);
  const { entry } = await appWithEntry({ window: { width: -100 } });

  expect(() => checkBuiltConfig(entry, makeSink().stream)).toThrow(
    expect.objectContaining({ code: "ERR_INCA_CONFIG_INVALID" }),
  );
});

it("checkBuiltConfig rethrows another error", async () => {
  mockedResolveHostBin.mockImplementationOnce(() => {
    throw new Error("unexpected");
  });
  const { entry } = await appWithEntry({});

  expect(() => checkBuiltConfig(entry, makeSink().stream)).toThrow("unexpected");
});
