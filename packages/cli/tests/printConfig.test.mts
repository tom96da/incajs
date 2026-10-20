// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

import { afterAll, beforeAll, expect, it, onTestFinished, vi } from "vitest";

import { printConfig } from "../src/printConfig.mts";
import { scratchConfigApp } from "./scratchConfigApp.mts";
import { makeSink, makeUnspawnableHost } from "./support/dev.mts";

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

it("hands the host an empty config for an app that declares nothing", async () => {
  const app = await scratch.makeApp({});
  const dir = path.join(app, "node_modules/.inca/config");
  await mkdir(dir, { recursive: true });
  await writeFile(path.join(dir, "inca.json"), JSON.stringify({ name: "earlier" }));

  const { config } = JSON.parse((await run(app)).stdout);

  expect(config).toEqual({});
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
