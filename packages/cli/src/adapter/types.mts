// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/** A running build watch — the value {@link Bundler.watch} resolves to. */
export interface Watcher {
  /** Stops watching and releases the underlying build process. */
  close(): Promise<void>;
}

/** Options for {@link Bundler.watch}. */
export interface BundlerOptions {
  /** The app's own entry point — may import `.vue` files. */
  entry: string;
  /** Where the build's output is written. */
  outDir: string;
  /**
   * Values the built app carries for `inca-host` to read — its name, id
   * and window. Any JSON-serializable object.
   */
  runtimeConfig?: unknown;
  /** `"development"` keeps a framework's own warnings; `"production"` strips them. */
  mode: "development" | "production";
  /** Called after each successful (re)build, with what it wrote. */
  onBuild: (output: BuildOutput) => void;
  /** Called instead of `onBuild` when a (re)build fails. */
  onError: (error: { message: string; stack: string | null; code?: string | null }) => void;
  /** Where the bundler's own output goes. Defaults to `process.stdout`. */
  stdout?: NodeJS.WritableStream;
  /** Where the bundler's warnings and errors go. Defaults to `process.stderr`. */
  stderr?: NodeJS.WritableStream;
  /**
   * Drops the bundler's progress output and size report, leaving the
   * per-build summary. Set it when the build has to stay silent: part of
   * that output goes to the process's own stdout, not to `stdout` above.
   */
  quiet?: boolean;
}

/** Options for {@link Bundler.build}. */
export interface BuildOptions {
  /** The app's own entry point — may import `.vue` files. */
  entry: string;
  /** Where the build's output is written. */
  outDir: string;
  /**
   * Values the built app carries for `inca-host` to read — its name, id
   * and window. Any JSON-serializable object.
   */
  runtimeConfig?: unknown;
  /** Where the bundler's own output goes. Defaults to `process.stdout`. */
  stdout?: NodeJS.WritableStream;
  /** Where the bundler's warnings and errors go. Defaults to `process.stderr`. */
  stderr?: NodeJS.WritableStream;
  /**
   * Drops the bundler's progress output and size report, leaving the
   * per-build summary. Set it when the build has to stay silent: part of
   * that output goes to the process's own stdout, not to `stdout` above.
   */
  quiet?: boolean;
}

/** What a build wrote — the result of {@link Bundler.build}, and every {@link BundlerOptions.onBuild} call. */
export interface BuildOutput {
  /** Directory holding every file this build emitted. */
  outDir: string;
  /** Absolute path to the entry module `inca-host` evaluates. */
  entryFile: string;
  /** Every file this build emitted, relative to `outDir` — may include chunks and assets besides the entry itself. */
  files: readonly string[];
  /** The change that triggered this rebuild. Absent from a one-shot build and a watch's first build. */
  changed?: { file: string; at: number };
}

/**
 * The contract a bundler adapter satisfies. Supporting another bundler
 * means adding a sibling adapter module and injecting its `Bundler` here —
 * never editing this package's own types.
 */
export interface Bundler {
  watch(options: BundlerOptions): Promise<Watcher>;
  /** One-shot production build, used by `inca build` — rejects on failure. */
  build(options: BuildOptions): Promise<BuildOutput>;
}
