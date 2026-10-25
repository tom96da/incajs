<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<!--
  Vue port of crates/inca-gpui/examples/hello_world.rs: a static,
  non-interactive tree — no reactivity needed since nothing changes after
  mount.

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
const colors = ["#ff0000", "#00ff00", "#0000ff", "#ffff00", "#000000", "#ffffff"];
</script>

<template>
  <div
    :style="{
      display: 'flex',
      flexDirection: 'column',
      gap: 12,
      width: 500,
      height: 500,
      justifyContent: 'center',
      alignItems: 'center',
      backgroundColor: '#505050',
      borderWidth: 1,
      borderColor: '#0000ff',
      color: '#ffffff',
      fontSize: 20,
    }"
  >
    {{ "Hello, Incarnative.js!" }}
    <div :style="{ display: 'flex', flexDirection: 'row', gap: 8 }">
      <div
        v-for="color in colors"
        :key="color"
        :style="{
          width: 32,
          height: 32,
          backgroundColor: color,
          borderWidth: 1,
          borderColor: '#ffffff',
          borderRadius: 4,
        }"
      />
    </div>
  </div>
</template>
