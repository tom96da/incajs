// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/** A running build watch — the value {@link Bundler.watch} resolves to. */
export interface Watcher {
  /** Where the bundle is written — read this rather than assuming a name. */
  bundlePath: string;
  /** Stops watching and releases the underlying build process. */
  close(): Promise<void>;
}

/** Options for {@link Bundler.watch}. */
export interface BundlerOptions {
  /** The app's own entry point — may import `.vue` files. */
  entry: string;
  /** Where the self-contained bundle is written — see `Watcher.bundlePath` for the exact file. */
  outDir: string;
  /** `"development"` keeps a framework's own warnings; `"production"` strips them. */
  mode: "development" | "production";
  /** Called after each successful (re)build, with the path to the freshly written bundle. */
  onBuild: (bundlePath: string) => void;
  /** Called instead of `onBuild` when a (re)build fails. */
  onError: (error: { message: string; stack: string | null }) => void;
}

/** Options for {@link Bundler.build}. */
export interface BuildOptions {
  /** The app's own entry point — may import `.vue` files. */
  entry: string;
  /** Where the self-contained bundle is written — see `BuildResult.bundlePath` for the exact file. */
  outDir: string;
}

/** The result of a one-shot {@link Bundler.build}. */
export interface BuildResult {
  /** Where the bundle was written — read this rather than assuming a name. */
  bundlePath: string;
}

/**
 * The contract a bundler adapter satisfies. Supporting another bundler
 * means adding a sibling adapter module and injecting its `Bundler` here —
 * never editing this package's own types.
 */
export interface Bundler {
  watch(options: BundlerOptions): Promise<Watcher>;
  /** One-shot production build, used by `inca build` — rejects on failure. */
  build(options: BuildOptions): Promise<BuildResult>;
}
