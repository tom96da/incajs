// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import type { Plugin } from "vite";

import { IncaError } from "../../error.mts";

/** Something an app can write that the engine can't run yet. */
export interface UnsupportedFeature {
  /** The `ERR_INCA_*` identifier this failure is looked up by. */
  code: string;
  /** The feature, named as the app's author would recognise it. */
  name: string;
  /** Whether a module id is a use of the feature. */
  matches: (id: string) => boolean;
  /** What happens instead, and what to reach for — appended to the error. */
  hint: string;
}

const STYLESHEET_RE = /\.(?:css|pcss|postcss|sass|scss|less|styl|stylus)(?:\?|$)/;

/**
 * Every feature a build refuses. Add an entry when the engine is found to
 * ignore something standard, so an app hits a build error instead of a
 * silent no-op at runtime; drop it again once the engine handles it.
 */
export const UNSUPPORTED: readonly UnsupportedFeature[] = [
  {
    code: "ERR_INCA_UNSUPPORTED_STYLE",
    name: "a <style> block",
    matches: (id) => id.includes("vue&type=style"),
    hint: "the renderer applies no stylesheet — use a :style binding instead",
  },
  {
    code: "ERR_INCA_UNSUPPORTED_STYLESHEET",
    name: "a stylesheet import",
    matches: (id) => STYLESHEET_RE.test(id),
    hint: "the renderer applies no stylesheet — use a :style binding instead",
  },
];

/**
 * Fails the build on the first module that uses an unsupported feature,
 * naming the file that used it.
 */
export function rejectUnsupported(
  features: readonly UnsupportedFeature[] = UNSUPPORTED,
): Plugin {
  return {
    name: "inca:reject-unsupported",
    enforce: "pre",
    transform(_code, id) {
      const feature = features.find((candidate) => candidate.matches(id));
      if (!feature) return null;
      const file = path.relative(process.cwd(), id.split("?")[0] ?? id);
      // The code is repeated in the message: the bundler re-wraps the
      // error and keeps only its text.
      throw new IncaError(
        feature.code,
        `${feature.code}: ${file} uses ${feature.name}, unsupported for now: ${feature.hint}`,
      );
    },
  };
}
