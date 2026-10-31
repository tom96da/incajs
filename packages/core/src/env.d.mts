// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/**
 * Replaced by the bundler at build time. Core runs in QuickJS, which has no `process`.
 */
declare const process: { env: { NODE_ENV?: string } };

/** The `DOMException` that QuickJS provides. */
declare const DOMException: new (message?: string, name?: string) => Error;
