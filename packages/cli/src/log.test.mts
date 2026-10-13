// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { Writable } from "node:stream";

import { describe, expect, it } from "vitest";

import { log, printFault, toFault, warn } from "./log.mts";

function sink(): { stream: Writable; text: () => string } {
  const chunks: string[] = [];
  return {
    stream: new Writable({
      write(chunk, _encoding, callback) {
        chunks.push(String(chunk));
        callback();
      },
    }),
    text: () => chunks.join(""),
  };
}

describe("log", () => {
  it("tags a line and leaves it unstamped by default", () => {
    const out = sink();

    log(out.stream, "built dist/bundle.js");

    expect(out.text()).toBe("[inca] built dist/bundle.js\n");
  });

  it("stamps the line with the time when asked", () => {
    const out = sink();

    log(out.stream, "ready", { timestamp: true });

    // The clock follows the runner's locale, so only the shape is asserted.
    expect(out.text()).toMatch(/^\d.* \[inca\] ready\n$/);
  });
});

describe("warn", () => {
  it("tags a line like log does", () => {
    const out = sink();

    warn(out.stream, "dev ignores the flag");

    expect(out.text()).toBe("[inca] dev ignores the flag\n");
  });

  it("colors the tag yellow on a terminal", () => {
    const chunks: string[] = [];
    const stream = Object.assign(
      new Writable({
        write(chunk, _encoding, callback) {
          chunks.push(String(chunk));
          callback();
        },
      }),
      { isTTY: true, hasColors: () => true },
    );

    warn(stream, "dev ignores the flag");

    expect(chunks.join("")).toContain("\u001b[33m");
  });
});

describe("printFault", () => {
  it("names the code when the failure carries one", () => {
    const out = sink();

    printFault(
      out.stream,
      "dev failed",
      toFault(Object.assign(new Error("busy"), { code: "ERR_INCA_DEV_RUNNING" })),
    );

    expect(out.text()).toContain("[inca] dev failed (ERR_INCA_DEV_RUNNING): busy\n");
  });

  it("prints the frames it doesn't already repeat, then the message", () => {
    const out = sink();

    printFault(out.stream, "build failed", toFault(new Error("syntax error")));

    const text = out.text();
    expect(text).toContain("[inca] build failed: syntax error\n");
    expect(text).toContain("    at ");
    expect(text).not.toContain("Error: syntax error\n    at");
    expect(text.indexOf("    at ")).toBeLessThan(text.indexOf("[inca] build failed"));
  });
});
