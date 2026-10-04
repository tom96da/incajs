// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { stripVTControlCharacters } from "node:util";

import { describe, expect, it } from "vitest";

import { faultOfUpdate } from "../../../src/adapter/vite/hmr.mts";
import { UNSUPPORTED } from "../../../src/adapter/vite/unsupported.mts";
import * as adapterCore from "../../../src/adapterCore.mts";

/** The fault with colour removed from its stack. */
function plain(error: Parameters<typeof faultOfUpdate>[1]) {
  const fault = faultOfUpdate(adapterCore, error);
  return { ...fault, stack: fault.stack && stripVTControlCharacters(fault.stack) };
}

describe("faultOfUpdate", () => {
  it("keeps the message and has no stack when there is nothing else", () => {
    expect(plain({ message: "boom" })).toEqual({ message: "boom", stack: null, code: null });
  });

  it("uses the stack alone when there is no plugin detail", () => {
    expect(plain({ message: "boom", stack: "Error: boom\n    at x" })).toEqual({
      message: "boom",
      stack: "Error: boom\n    at x",
      code: null,
    });
  });

  it("filters out an empty stack", () => {
    expect(plain({ message: "boom", stack: "" })).toEqual({
      message: "boom",
      stack: null,
      code: null,
    });
  });

  it("filters out a null stack", () => {
    expect(plain({ message: "boom", stack: null, plugin: "p" }).stack).toBe("  Plugin: p");
  });

  it("appends plugin, file with position and frame after the stack, in order", () => {
    const { stack } = plain({
      message: "boom",
      stack: "at x",
      plugin: "vite:vue",
      id: "/app/App.vue",
      loc: { line: 3, column: 7 },
      frame: "1 | a\n2 | b",
    });
    const lines = stack!.split("\n");

    expect(lines[0]).toBe("at x");
    expect(stack).toContain("Plugin: vite:vue");
    expect(stack).toContain("File: /app/App.vue:3:7");
    expect(stack!.indexOf("Plugin:")).toBeLessThan(stack!.indexOf("File:"));
    expect(stack!.indexOf("File:")).toBeLessThan(stack!.indexOf("1 | a"));
    expect(stack).toContain("  1 | a\n  2 | b");
  });

  it("omits the :line:col suffix when the id has no loc", () => {
    const { stack } = plain({ message: "boom", id: "/app/App.vue" });

    expect(stack).toContain("File: /app/App.vue");
    expect(stack).not.toMatch(/App\.vue:\d/);
  });

  it("lifts an ERR_INCA code out of the message, as a failed rebuild does", () => {
    const message = "Error: ERR_INCA_X: a.vue uses a thing";

    expect(plain({ message })).toEqual({
      message: "a.vue uses a thing",
      stack: null,
      code: "ERR_INCA_X",
    });
    expect(plain({ message })).toMatchObject(adapterCore.bundlerFault(message));
  });

  it("strips an error-class prefix when there is no code", () => {
    expect(plain({ message: "TypeError: bad" })).toEqual({
      message: "bad",
      stack: null,
      code: null,
    });
  });

  it("keeps a message with neither prefix nor code as it is", () => {
    expect(plain({ message: "just text" }).message).toBe("just text");
  });

  it("reports the failure unsupported.mts raises the way a rebuild does", () => {
    const feature = UNSUPPORTED[0]!;
    const message = `${feature.code}: App.vue uses ${feature.name}, unsupported for now: ${feature.hint}`;
    const fault = plain({ message, stack: "at x" });

    expect(fault.code).toBe(feature.code);
    expect(fault.message).toBe(adapterCore.bundlerFault(message).message);
    expect(fault.message).not.toContain("ERR_INCA");
    expect(fault.stack).toBe("at x");
  });
});
