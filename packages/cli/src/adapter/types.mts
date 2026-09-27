// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/** A running build watch — the value {@link Bundler.watch} resolves to. */
export interface Watcher {
  /** Stops watching and releases the underlying build process. */
  close(): Promise<void>;
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

/** Options for {@link Bundler.watch}. */
export interface BundlerOptions extends BuildOptions {
  /** `"development"` keeps a framework's own warnings; `"production"` strips them. */
  mode: "development" | "production";
  /** Called after each successful (re)build, with what it wrote. */
  onBuild: (output: BuildOutput) => void;
  /** Called instead of `onBuild` when a (re)build fails. */
  onError: (error: { message: string; stack: string | null; code?: string | null }) => void;
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
 * A failed HMR update's detail — the same fields a Rollup/Vite plugin
 * error carries beyond `message`/`stack`, when it has them.
 */
export interface UpdateError {
  message: string;
  stack?: string | null;
  plugin?: string | null;
  id?: string | null;
  frame?: string | null;
  loc?: { line: number; column: number } | null;
}

/** Options for {@link Bundler.hmr}. */
export interface HmrOptions {
  /** The app's own entry point — may import `.vue` files. */
  entry: string;
  /** The app's root directory — where its dev-only scratch files are written. */
  cwd: string;
  /**
   * Values the built app carries for `inca-host` to read — its name, id
   * and window. Any JSON-serializable object.
   */
  runtimeConfig?: unknown;
  /** Overrides where the synthesized entry imports `hmr-runtime.js` from. Test-only. */
  runtimePath?: string;
  /** Where the bundler's own output goes. Defaults to `process.stdout`. */
  stdout?: NodeJS.WritableStream;
  /** Where the bundler's warnings and errors go. Defaults to `process.stderr`. */
  stderr?: NodeJS.WritableStream;
  /** Drops per-update/reload logging; warnings and errors still surface. */
  quiet?: boolean;
  /** Delivers a bundler payload to the running app over the dev channel. */
  notify: (payload: unknown) => void;
  /** Called in place of forwarding a whole-app reload payload to the app. */
  reload: () => void;
  /** Called when an update the app fetched failed to compile. */
  onError: (error: UpdateError) => void;
  /** Called after a successful update, with the file that changed and how long it took. */
  onUpdate?: (info: { file: string; took: number }) => void;
}

/** A running HMR session — the value {@link Bundler.hmr} resolves to. */
export interface HmrChannel extends Watcher {
  /** The synthesized entry `inca-host` evaluates for this session. */
  entryFile: string;
  /** Feeds a payload the running app sent back into the bundler's dev server. */
  dispatch: (payload: unknown) => void;
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
  /**
   * Module-granular dev updates, used by `inca dev --experimental-hmr`.
   * Optional — a bundler adapter with no HMR support simply omits it.
   */
  hmr?(options: HmrOptions): Promise<HmrChannel>;
}
