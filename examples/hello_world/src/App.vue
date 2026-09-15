<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<!--
  Vue port of crates/inca-gpui/examples/hello_world.rs's build_tree(): a
  static, non-interactive tree — no reactivity needed, matching the Rust
  original's own choice of plain `render_tree` with no event dispatch.

  The label below is a mustache interpolation, not plain template text:
  inca's "text" nodes render their value verbatim (no trimming), and
  Vue's whitespace condensing only fully removes whitespace-only text
  between two elements — the interpolation's own trailing edge still picks
  up one condensed space before the following <div>. That's expected,
  harmless real-Vue-template behavior (any multi-line template does this),
  not something to work around — do not add a comment or extra element
  inside <template> to try to fix it, either would add a real (albeit
  hidden/empty) node to the rendered tree.
-->
<script setup>
const colors = [0xff0000, 0x00ff00, 0x0000ff, 0xffff00, 0x000000, 0xffffff];
</script>

<template>
  <div
    :style="{
      display: 'flex',
      flex_direction: 'column',
      gap: 12,
      width: 500,
      height: 500,
      justify_content: 'center',
      align_items: 'center',
      background: 0x505050,
      border_width: 1,
      border_color: 0x0000ff,
      text_color: 0xffffff,
      text_size: 20,
    }"
  >
    {{ "Hello, Incarnative.js!" }}
    <div :style="{ display: 'flex', flex_direction: 'row', gap: 8 }">
      <div
        v-for="color in colors"
        :key="color"
        :style="{
          width: 32,
          height: 32,
          background: color,
          border_width: 1,
          border_color: 0xffffff,
          corner_radius: 4,
        }"
      />
    </div>
  </div>
</template>
