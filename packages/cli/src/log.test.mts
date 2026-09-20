// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { Writable } from "node:stream";

import { describe, expect, it } from "vitest";

import { log, printFault, toFault } from "./log.mts";

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

  it("prints the message, then the frames it doesn't already repeat", () => {
    const out = sink();

    printFault(out.stream, "build failed", toFault(new Error("syntax error")));

    expect(out.text()).toContain("[inca] build failed: syntax error\n");
    expect(out.text()).toContain("    at ");
    expect(out.text()).not.toContain("Error: syntax error\n    at");
  });
});
