#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Keeps the generated regions of NOTICE complete. The crates are the dependencies of the
// host binary on the four platforms. Each entry takes its facts from Cargo.toml and its
// text from the license files of the crate.
//
// Usage: node scripts/generate-notice.mts [--check]
// Without a flag the script rewrites NOTICE. `--check` writes nothing and exits 1 when
// NOTICE, or the `license` of a platform package, is out of date.

import { execFile } from "node:child_process";
import { readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";

/**
 * Orders two strings by code unit.
 *
 * @param a - the first string.
 * @param b - the second string.
 * @returns a negative number, 0 or a positive number.
 */
function compare(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

/**
 * Gives a license text LF line endings, no trailing space and no final newline.
 *
 * @param text - a license file content.
 * @returns the normalized text.
 */
function normalizeText(text: string): string {
  return text
    .replaceAll(/\r\n?/g, "\n")
    .replaceAll(/[ \t]+$/gm, "")
    .replace(/\n+$/, "");
}

const PERMISSIVE = new Set(["MIT", "Apache-2.0"]);

/**
 * Reads an SPDX license expression. The legacy `/` reads as OR and a `WITH` exception belongs to
 * its id.
 *
 * @param expression - the `license` field of a crate.
 * @returns the sorted ids other than MIT and Apache-2.0 when those two do not satisfy the
 *   expression, and an empty array when they do.
 */
function unsatisfiedIds(expression: string): string[] {
  const tokens = expression.replaceAll("/", " OR ").match(/[()]|[^\s()]+/g) ?? [];
  const ids = new Set<string>();
  let pos = 0;

  const atom = (): boolean => {
    const token = tokens[pos++];
    if (token === "(") {
      const satisfied = or();
      if (tokens[pos++] !== ")") throw new Error(`invalid license expression: ${expression}`);
      return satisfied;
    }
    if (token === undefined || token === ")" || token === "AND" || token === "OR") {
      throw new Error(`invalid license expression: ${expression}`);
    }
    if (tokens[pos] === "WITH") pos += 2;
    ids.add(token);
    return PERMISSIVE.has(token);
  };
  const and = (): boolean => {
    let satisfied = atom();
    while (tokens[pos] === "AND") {
      pos++;
      satisfied = atom() && satisfied;
    }
    return satisfied;
  };
  const or = (): boolean => {
    let satisfied = and();
    while (tokens[pos] === "OR") {
      pos++;
      satisfied = and() || satisfied;
    }
    return satisfied;
  };

  const satisfied = or();
  if (pos !== tokens.length) throw new Error(`invalid license expression: ${expression}`);
  if (satisfied) return [];
  return [...ids].filter((id) => !PERMISSIVE.has(id)).sort();
}

const run = promisify(execFile);

/** The targets of the four platform packages. */
const TARGETS = [
  "x86_64-unknown-linux-gnu",
  "aarch64-unknown-linux-gnu",
  "aarch64-apple-darwin",
  "x86_64-apple-darwin",
] as const;

interface Crate {
  name: string;
  version: string;
  license: string;
  /** The directory of the crate's manifest. */
  dir: string;
  repository: string | null;
  authors: string[];
  licenseFile: string | null;
  /** The targets whose release build links the crate. */
  targets: string[];
}

interface MetadataPackage {
  name: string;
  version: string;
  manifest_path: string;
  repository: string | null;
  authors: string[];
  license_file: string | null;
}

async function cargo(root: string, args: string[]): Promise<string> {
  try {
    return (await run("cargo", args, { cwd: root, maxBuffer: 1 << 30 })).stdout;
  } catch (error) {
    const { stderr } = error as { stderr?: string };
    throw new Error(stderr?.trim() || `cargo ${args[0]} failed`, { cause: error });
  }
}

/**
 * Lists the dependencies of the host binary on any of the four targets.
 *
 * @param root - the repository root.
 * @returns the crates sorted by name, then version.
 */
async function readCrates(root: string): Promise<Crate[]> {
  await cargo(root, ["fetch", "--locked"]);
  const metadata: { packages: MetadataPackage[] } = JSON.parse(
    await cargo(root, ["metadata", "--locked", "--format-version", "1"]),
  );
  const packages = new Map<string, MetadataPackage>();
  for (const pkg of metadata.packages) {
    const key = `${pkg.name} ${pkg.version}`;
    if (packages.has(key)) throw new Error(`cargo metadata lists ${key} twice`);
    packages.set(key, pkg);
  }

  const crates = new Map<string, Crate>();
  for (const target of TARGETS) {
    const tree = await cargo(root, [
      "tree",
      "--locked",
      "-p",
      "inca-host",
      "-e",
      "normal",
      "--prefix",
      "none",
      "--no-dedupe",
      "--target",
      target,
      "-f",
      "{p}|{l}",
    ]);
    for (const line of tree.split("\n")) {
      const match = /^(\S+) v(\S+)[^|]*\|(.*)$/.exec(line);
      if (!match) continue;
      const [, name = "", version = "", license = ""] = match;
      const key = `${name} ${version}`;
      const known = crates.get(key);
      if (known) {
        if (!known.targets.includes(target)) known.targets.push(target);
        continue;
      }
      const pkg = packages.get(key);
      if (!pkg) throw new Error(`cargo metadata does not list ${key}`);
      if (!license) throw new Error(`${key} has no license field`);
      crates.set(key, {
        name,
        version,
        license,
        dir: pkg.manifest_path.replace(/[/\\][^/\\]*$/, ""),
        repository: pkg.repository,
        authors: pkg.authors,
        licenseFile: pkg.license_file,
        targets: [target],
      });
    }
  }
  return [...crates.values()].sort(
    (a, b) => compare(a.name, b.name) || compare(a.version, b.version),
  );
}

/** The file with the terms of a license, for a crate that holds several license files. */
const FILE_OF_ID: Record<string, string> = {
  "BSD-3-Clause": "LICENSE-WHATWG",
  "Unicode-3.0": "LICENSE-UNICODE",
};

/**
 * Reads the license file of a crate.
 *
 * @param crate - the crate.
 * @param id - the license id whose terms the file holds.
 * @returns the file content, or null when the crate has no license file.
 */
async function findLicenseText(crate: Crate, id: string): Promise<string | null> {
  let name = crate.licenseFile;
  if (name === null) {
    const files = (await readdir(crate.dir, { withFileTypes: true }))
      .filter((entry) => entry.isFile() && /^(license|copying|copyright)/i.test(entry.name))
      .map((entry) => entry.name);
    if (files.length === 0) return null;
    if (files.length === 1) {
      name = files[0] ?? null;
    } else {
      name = files.find((file) => file === FILE_OF_ID[id]) ?? null;
      if (name === null) {
        throw new Error(`${crate.name} ${crate.version} has several license files, none for ${id}`);
      }
    }
  }
  return readFile(path.join(crate.dir, name ?? ""), "utf8");
}

/** The licenses that may appear in an entry. The text of MPL-2.0 is written by hand. */
const ALLOWED_IDS = new Set([
  "BSD-2-Clause",
  "BSD-3-Clause",
  "CC0-1.0",
  "ISC",
  "MPL-2.0",
  "Unicode-3.0",
  "Zlib",
]);

interface Sections {
  /** The entries, one per crate without an MIT or Apache-2.0 option. */
  entries: string;
  /** The license texts, one block per distinct text. */
  texts: string;
  /** The ids of the entries of each target. */
  idsByTarget: Map<string, Set<string>>;
}

interface Block {
  id: string;
  heading: string;
  text: string;
  users: Crate[];
}

const rule = (title: string): string => `------- ${title} --------`;

const byNameVersion = (a: Crate, b: Crate): number =>
  compare(a.name, b.name) || compare(a.version, b.version);

/**
 * Builds the generated entries and license texts of NOTICE.
 *
 * @param crates - the crates of the release binary.
 * @returns the entries, the license texts and the license ids of each target.
 */
async function buildSections(crates: Crate[]): Promise<Sections> {
  const entries = crates.flatMap((crate) => {
    const ids = unsatisfiedIds(crate.license);
    for (const id of ids) {
      if (!ALLOWED_IDS.has(id)) {
        throw new Error(`${crate.name} ${crate.version} has the license ${id}, which has no rule`);
      }
    }
    return ids.length > 0 ? [{ crate, ids }] : [];
  });

  const idsByTarget = new Map<string, Set<string>>();
  for (const { crate, ids } of entries) {
    for (const target of crate.targets) {
      const set = idsByTarget.get(target) ?? new Set<string>();
      for (const id of ids) set.add(id);
      idsByTarget.set(target, set);
    }
  }

  // Each text is looked up once. A crate without a file takes the text of
  // the first crate with the id.
  const texts = new Map<string, string | null>();
  for (const { crate, ids } of entries) {
    for (const id of ids.filter((id) => id !== "MPL-2.0")) {
      const text = await findLicenseText(crate, id);
      texts.set(`${crate.name} ${crate.version} ${id}`, text === null ? null : normalizeText(text));
    }
  }
  const blocks = new Map<string, Block>();
  const copyrightOf = new Map<Crate, string>();
  for (const { crate, ids } of entries) {
    for (const id of ids.filter((id) => id !== "MPL-2.0")) {
      let text = texts.get(`${crate.name} ${crate.version} ${id}`) ?? null;
      if (text === null) {
        const donor = entries.find(
          (other) =>
            other.ids.includes(id) && texts.get(`${other.crate.name} ${other.crate.version} ${id}`),
        );
        text = donor
          ? (texts.get(`${donor.crate.name} ${donor.crate.version} ${id}`) ?? null)
          : null;
        if (text === null) throw new Error(`${crate.name} ${crate.version} has no license file`);
      }
      const holder = text.split("\n").find((line) => line.startsWith("Copyright"));
      if (holder !== undefined && !copyrightOf.has(crate)) copyrightOf.set(crate, holder);
      const key = `${id}\n${text}`;
      const block = blocks.get(key) ?? {
        id,
        heading: holder === undefined ? id : `${id}: ${holder}`,
        text,
        users: [],
      };
      block.users.push(crate);
      blocks.set(key, block);
    }
  }

  const entryText = entries.map(({ crate }) => {
    const authors = crate.authors.map((author) => author.replace(/\s*<[^>]*>/, "")).join(", ");
    return [
      rule(crate.name),
      `Version: ${crate.version}`,
      `License: ${crate.license}`,
      ...(copyrightOf.has(crate) ? [`Copyright: ${copyrightOf.get(crate)}`] : []),
      ...(authors ? [`Authors: ${authors}`] : []),
      ...(crate.repository ? [`Source: ${crate.repository}`] : []),
    ].join("\n");
  });

  const blockText = [...blocks.values()]
    .sort((a, b) => compare(a.id, b.id) || compare(a.heading, b.heading))
    .map((block) => {
      const used = block.users.sort(byNameVersion).map((c) => `${c.name} ${c.version}`);
      return `${rule(block.heading)}\nUsed by: ${used.join(", ")}\n\n${block.text}`;
    });

  return {
    entries: entryText.join("\n\n"),
    texts: blockText.join("\n\n"),
    idsByTarget,
  };
}

const MARKERS = [
  "##### BEGIN GENERATED ENTRIES #####",
  "##### END GENERATED ENTRIES #####",
  "##### BEGIN GENERATED TEXTS #####",
  "##### END GENERATED TEXTS #####",
] as const;

const MPL_HEADING = "------- Mozilla Public License 2.0 --------";

/**
 * Replaces the two generated regions of NOTICE and copies the rest unchanged.
 *
 * @param current - the content of NOTICE.
 * @param entries - the text between the entry markers.
 * @param texts - the text between the license text markers.
 * @returns the new content and the hand-written MPL-2.0 text.
 */
function replaceSections(
  current: string,
  entries: string,
  texts: string,
): { content: string; mpl: string } {
  const lines = current.split("\n");
  const at = MARKERS.map((marker) => {
    const found = lines.flatMap((line, index) => (line === marker ? [index] : []));
    if (found.length !== 1) throw new Error(`NOTICE needs the line ${marker} exactly once`);
    return found[0] ?? 0;
  });
  const [beginEntries = 0, endEntries = 0, beginTexts = 0, endTexts = 0] = at;
  if (at.some((index, i) => i > 0 && index < (at[i - 1] ?? 0))) {
    throw new Error(`NOTICE needs its markers in the order ${MARKERS.join(", ")}`);
  }

  const heading = lines.indexOf(MPL_HEADING, endEntries);
  if (heading < 0 || heading > beginTexts) throw new Error(`NOTICE needs the line ${MPL_HEADING}`);

  const content = [
    ...lines.slice(0, beginEntries + 1),
    "",
    ...entries.split("\n"),
    "",
    ...lines.slice(endEntries, beginTexts + 1),
    "",
    ...texts.split("\n"),
    "",
    ...lines.slice(endTexts),
  ].join("\n");
  return {
    content,
    mpl: lines
      .slice(heading + 1, beginTexts)
      .join("\n")
      .trim(),
  };
}

const PLATFORMS: Record<string, string> = {
  "npm/darwin-arm64": "aarch64-apple-darwin",
  "npm/darwin-x64": "x86_64-apple-darwin",
  "npm/linux-arm64": "aarch64-unknown-linux-gnu",
  "npm/linux-x64": "x86_64-unknown-linux-gnu",
};

/**
 * Compares the `license` of each platform package with the licenses of its libraries.
 *
 * @param root - the repository root.
 * @param idsByTarget - the license ids of the NOTICE entries of each target.
 * @returns one message per package whose `license` differs from the expected expression.
 */
async function checkPlatforms(
  root: string,
  idsByTarget: Map<string, Set<string>>,
): Promise<string[]> {
  const problems: string[] = [];
  for (const [dir, target] of Object.entries(PLATFORMS)) {
    const expected = ["(MIT OR Apache-2.0)", ...[...(idsByTarget.get(target) ?? [])].sort()].join(
      " AND ",
    );
    const manifest: { license?: string } = JSON.parse(
      await readFile(path.join(root, dir, "package.json"), "utf8"),
    );
    if (manifest.license !== expected) {
      problems.push(`${dir}: license is ${manifest.license}. expected ${expected}`);
    }
  }
  return problems;
}

const root = path.resolve(import.meta.dirname, "..");
const noticePath = path.join(root, "NOTICE");

/** Runs the command and returns the problems it found. */
async function main(): Promise<string[]> {
  const args = process.argv.slice(2);
  const check = args[0] === "--check";
  if (args.length > (check ? 1 : 0)) throw new Error(`unknown argument: ${args.join(" ")}`);

  const crates = await readCrates(root);
  const { entries, texts, idsByTarget } = await buildSections(crates);
  const current = await readFile(noticePath, "utf8");
  const { content, mpl } = replaceSections(current, entries, texts);
  if (!check) {
    await writeFile(noticePath, content);
    return [];
  }

  const problems = await checkPlatforms(root, idsByTarget);
  const cssparser = crates.find((crate) => crate.name === "cssparser");
  if (!cssparser) throw new Error("cssparser is not linked");
  const source = await readFile(path.join(cssparser.dir, "LICENSE"), "utf8");
  if (normalizeText(mpl) !== normalizeText(source)) {
    problems.push("the MPL-2.0 text of NOTICE differs from the license file of cssparser");
  }
  if (content !== current) {
    const expected = content.split("\n");
    const actual = current.split("\n");
    let line = 0;
    const length = Math.max(expected.length, actual.length);
    while (line < length && expected[line] === actual[line]) line++;
    problems.unshift(
      `NOTICE is out of date. run make notice\nline ${line + 1}: expected ${expected[line]}\nline ${line + 1}: found ${actual[line]}`,
    );
  }
  return problems;
}

try {
  const problems = await main();
  for (const problem of problems) console.error(problem);
  process.exitCode = problems.length > 0 ? 1 : 0;
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
