// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it } from "vitest";

import * as core from "../adapterCore.mts";
import { captureOutput } from "./captureOutput.mts";
import type { CapturedOutput } from "./captureOutput.mts";

type WriteBundle = (options: object, bundle: Record<string, object>) => void;

const entry = { type: "chunk", isEntry: true, fileName: "bundle.js" };
const chunk = { type: "chunk", isEntry: false, fileName: "chunks/a.js" };

function writeBundleOf(sink: (output: CapturedOutput) => void): WriteBundle {
  return captureOutput(core, sink).writeBundle as unknown as WriteBundle;
}

describe("captureOutput", () => {
  it("reports the entry chunk and every file of the bundle", () => {
    const captured: CapturedOutput[] = [];

    writeBundleOf((output) => captured.push(output))(
      {},
      { "bundle.js": entry, "chunks/a.js": chunk },
    );

    expect(captured).toEqual([{ entryFile: "bundle.js", files: ["bundle.js", "chunks/a.js"] }]);
  });

  it("throws the code in the message when no chunk is the entry", () => {
    expect(() => writeBundleOf(() => {})({}, { "chunks/a.js": chunk })).toThrow(
      expect.objectContaining({
        code: "ERR_INCA_BUILD_NO_ENTRY_CHUNK",
        message: "ERR_INCA_BUILD_NO_ENTRY_CHUNK: build produced no entry chunk",
      }),
    );
  });
});
