// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach } from "vitest";

import type { EditWatch } from "../../src/watchForEdit.mts";

type WatchModule = typeof import("../../src/watchForEdit.mts");

/** Every `watchForEdit` call of a test, in order. */
export const watches = {
  list: [] as { closed: boolean }[],
  /** Makes the next `watchForEdit` call throw, as when the OS refuses a watcher. */
  refuseNext: false,
  /** Whether every watch that was opened is closed again. */
  allClosed(): boolean {
    return this.list.every((watch) => watch.closed);
  },
};

beforeEach(() => {
  watches.list = [];
  watches.refuseNext = false;
});

/**
 * The module `vi.mock("../src/watchForEdit.mts")` is given, with each call recorded in `watches`.
 */
export function recordWatches(actual: WatchModule): WatchModule {
  return {
    ...actual,
    watchForEdit(cwd, options): EditWatch {
      if (watches.refuseNext) {
        watches.refuseNext = false;
        throw new Error("too many open files");
      }
      const watch = actual.watchForEdit(cwd, options);
      const record = { closed: false };
      watches.list.push(record);
      return {
        ...watch,
        close() {
          record.closed = true;
          watch.close();
        },
      };
    },
  };
}
