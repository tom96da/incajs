// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { expect, it } from "vitest";

import { defaultFrontend } from "./defaultPlugins.mts";

it("writes the Vue entry text for App.vue, with no trailing newline", () => {
  expect(defaultFrontend.entryCode('"/a/App.vue"')).toBe(
    'import { createIncaApp } from "incajs/vue";\nimport App from "/a/App.vue";\ncreateIncaApp(App).mount();',
  );
});
