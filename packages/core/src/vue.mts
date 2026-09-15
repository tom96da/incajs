// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The `incajs/vue` subpath: `@incajs/vue`'s renderer, pre-wired with this
// package's own bindings, for callers who'd rather not thread them through
// (or add `@incajs/vue` as a second explicit dependency) themselves.

import type { Component } from "@vue/runtime-core";

import * as core from "incajs";

import { createGpjsuiApp as createVueApp } from "@incajs/vue";

export function createGpjsuiApp(
  rootComponent: Component,
  rootProps?: Record<string, unknown> | null,
) {
  return createVueApp(core, rootComponent, rootProps);
}

export type { GpjsuiApp, GpjsuiElement, GpjsuiNode, IncajsCore } from "@incajs/vue";
