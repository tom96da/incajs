// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import * as core from "../adapterCore.mts";
import { captureOutput } from "./captureOutput.mts";
import type { CapturedOutput } from "./captureOutput.mts";

type WriteBundle = (options: object, bundle: Record<string, object>) => Promise<void>;

const entry = { type: "chunk", isEntry: true, fileName: "bundle.js" };
const chunk = { type: "chunk", isEntry: false, fileName: "chunks/a.js" };

function writeBundleOf(sink: (output: CapturedOutput) => void): WriteBundle {
  return captureOutput(core, sink).writeBundle as unknown as WriteBundle;
}

interface ResolvedPublic {
  publicDir: string;
  build: { copyPublicDir: boolean };
}

/** Runs one build with a resolved config and returns the reported files. */
async function filesOf(
  resolved: ResolvedPublic,
  bundle: Record<string, object>,
): Promise<readonly string[]> {
  const captured: CapturedOutput[] = [];
  const plugin = captureOutput(core, (output) => captured.push(output));
  (plugin.configResolved as unknown as (config: ResolvedPublic) => void)(resolved);
  await (plugin.writeBundle as unknown as WriteBundle)({}, bundle);
  return captured[0]?.files ?? [];
}

const dirs: string[] = [];

afterEach(async () => {
  await Promise.all(dirs.splice(0).map((dir) => rm(dir, { recursive: true, force: true })));
});

async function publicDirWith(...files: string[]): Promise<string> {
  const dir = await mkdtemp(path.join(tmpdir(), "inca-public-"));
  dirs.push(dir);
  for (const file of files) {
    await mkdir(path.dirname(path.join(dir, file)), { recursive: true });
    await writeFile(path.join(dir, file), "x");
  }
  return dir;
}

describe("captureOutput", () => {
  it("reports the entry chunk and every file of the bundle", async () => {
    const captured: CapturedOutput[] = [];

    await writeBundleOf((output) => captured.push(output))(
      {},
      { "bundle.js": entry, "chunks/a.js": chunk },
    );

    expect(captured).toEqual([{ entryFile: "bundle.js", files: ["bundle.js", "chunks/a.js"] }]);
  });

  it("throws the code in the message when no chunk is the entry", async () => {
    await expect(writeBundleOf(() => {})({}, { "chunks/a.js": chunk })).rejects.toThrow(
      expect.objectContaining({
        code: "ERR_INCA_BUILD_NO_ENTRY_CHUNK",
        message: "ERR_INCA_BUILD_NO_ENTRY_CHUNK: build produced no entry chunk",
      }),
    );
  });

  it("lists the files of public with / separators", async () => {
    const publicDir = await publicDirWith("a/b.txt");

    expect(
      await filesOf({ publicDir, build: { copyPublicDir: true } }, { "bundle.js": entry }),
    ).toEqual(["bundle.js", "a/b.txt"]);
  });

  it("lists the files under a symlinked directory of public", async () => {
    const publicDir = await publicDirWith("real/a.txt");
    await symlink(path.join(publicDir, "real"), path.join(publicDir, "linkdir"));

    const files = await filesOf(
      { publicDir, build: { copyPublicDir: true } },
      { "bundle.js": entry },
    );

    expect([...files].sort()).toEqual(["bundle.js", "linkdir/a.txt", "real/a.txt"]);
  });

  it("keeps the siblings of an entry that cannot be inspected", async () => {
    const publicDir = await publicDirWith("a.txt");
    await symlink(path.join(publicDir, "missing"), path.join(publicDir, "dangling"));

    expect(
      await filesOf({ publicDir, build: { copyPublicDir: true } }, { "bundle.js": entry }),
    ).toEqual(["bundle.js", "a.txt"]);
  });

  it("lists the bundle files only when copyPublicDir is false", async () => {
    const publicDir = await publicDirWith("a.txt");

    expect(
      await filesOf({ publicDir, build: { copyPublicDir: false } }, { "bundle.js": entry }),
    ).toEqual(["bundle.js"]);
  });

  it("lists the bundle files only when the public directory is missing", async () => {
    const publicDir = path.join(await publicDirWith(), "missing");

    expect(
      await filesOf({ publicDir, build: { copyPublicDir: true } }, { "bundle.js": entry }),
    ).toEqual(["bundle.js"]);
  });

  it("lists a name emitted by the bundle and by public once", async () => {
    const publicDir = await publicDirWith("bundle.js");

    expect(
      await filesOf({ publicDir, build: { copyPublicDir: true } }, { "bundle.js": entry }),
    ).toEqual(["bundle.js"]);
  });
});
