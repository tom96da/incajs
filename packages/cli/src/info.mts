// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { version } from "../package.json" with { type: "json" };
import { IncaError } from "./error.mts";
import { assertHostBin, inspectHostPackage, resolveHostBin } from "./host/index.mts";

/** The host package of this platform, as `name@version` or a note on why there is none. */
async function hostPackageValue(): Promise<string> {
  const host = await inspectHostPackage();
  switch (host.state) {
    case "unsupported":
      return "unavailable (unsupported platform)";
    case "notInstalled":
      return `${host.target.name} (not installed)`;
    case "noBinary":
      return `${host.target.name} (no binary)`;
    case "corrupt":
      return `${host.target.name} (corrupt)`;
    case "installed":
      return `${host.target.name}@${host.pkg.version}`;
  }
}

/** The path of the host binary the other commands start. */
async function hostBinaryValue(cwd: string, stderr: NodeJS.WritableStream): Promise<string> {
  const bin = await resolveHostBin(cwd, stderr);
  await assertHostBin(bin);
  return bin;
}

/**
 * Runs `value` and turns an `IncaError` into the text of an unavailable line, such as
 * `unavailable (unpack failed)`.
 */
async function valueOrUnavailable(value: () => Promise<string>): Promise<string> {
  try {
    return await value();
  } catch (error) {
    if (!(error instanceof IncaError)) throw error;
    const reason = error.code
      .replace(/^ERR_INCA_HOST_/, "")
      .toLowerCase()
      .replaceAll("_", " ");
    return `unavailable (${reason})`;
  }
}

/**
 * Prints the version of Incarnative.js, the host package and the host binary, one aligned line
 * each. Unpacks the host binary when no directory holds it.
 * @param stdout - receives the lines
 * @param cwd - the app's root directory
 * @param stderr - receives the line that names the directory a new unpack wrote into
 */
export async function info(
  stdout: NodeJS.WritableStream = process.stdout,
  cwd: string = process.cwd(),
  stderr: NodeJS.WritableStream = process.stderr,
): Promise<void> {
  const rows: [string, string][] = [
    ["Incarnative.js:", version],
    ["host package:", await valueOrUnavailable(hostPackageValue)],
    ["host binary:", await valueOrUnavailable(() => hostBinaryValue(cwd, stderr))],
  ];
  const width = Math.max(...rows.map(([label]) => label.length)) + 1;
  stdout.write(rows.map(([label, value]) => `${label.padEnd(width)}${value}\n`).join(""));
}
