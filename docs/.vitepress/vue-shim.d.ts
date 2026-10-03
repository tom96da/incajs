// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

declare module "*.vue" {
  import type { DefineComponent } from "vue";

  const component: DefineComponent;
  export default component;
}
