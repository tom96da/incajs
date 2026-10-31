// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { createHash } from "node:crypto";
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { brotliCompressSync, constants } from "node:zlib";

import { onTestFinished } from "vitest";

import type { HostPackage } from "../../src/host/package.mts";

/**
 * Writes a host package into `dir`: the brotli file and the `incaHost` field. Quality 1 keeps
 * tests fast, and the decoder reads any quality.
 * @param dir - the package directory, created if missing
 * @param binary - the bytes of the unpacked binary
 * @param sha256 - the hash to record, when a test needs one that doesn't match `binary`
 * @returns the package as `readHostPackage` reads it
 */
export function writeHostPackage(dir: string, binary: Buffer, sha256?: string): HostPackage {
  const hash = sha256 ?? createHash("sha256").update(binary).digest("hex");
  mkdirSync(path.join(dir, "bin"), { recursive: true });
  writeFileSync(
    path.join(dir, "bin", "inca-host.br"),
    brotliCompressSync(binary, { params: { [constants.BROTLI_PARAM_QUALITY]: 1 } }),
  );
  writeFileSync(
    path.join(dir, "package.json"),
    JSON.stringify({
      name: "@incajs/host-linux-x64",
      version: "1.2.3",
      incaHost: { sha256: hash },
    }),
  );
  return { name: "@incajs/host-linux-x64", dir, version: "1.2.3", sha256: hash };
}

/** A new temporary directory, removed when the test finishes. */
export function temp(): string {
  const dir = mkdtempSync(path.join(tmpdir(), "inca-host-"));
  onTestFinished(() => {
    chmodSync(dir, 0o700);
    rmSync(dir, { recursive: true, force: true });
  });
  return dir;
}

/** A regular file, as a directory that can't be written. */
export function regularFile(): string {
  const file = path.join(temp(), "file");
  writeFileSync(file, "x");
  return file;
}
