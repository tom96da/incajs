<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<!--
  Vue port of crates/inca-bridge/examples/click_counter.rs: a clickable,
  bordered box whose label counts up — the JS-side reactivity/event-dispatch
  counterpart to hello_world's purely static tree.
-->
<script setup>
import { computed, ref } from "@vue/runtime-core";

const clicks = ref(0);
const label = computed(() =>
  clicks.value === 0 ? "Click me!" : `Clicked ${clicks.value} time(s)`,
);

function onClick() {
  clicks.value += 1;
}
</script>

<template>
  <div
    :style="{
      display: 'flex',
      justify_content: 'center',
      align_items: 'center',
      width: 300,
      height: 150,
      background: 0x505050,
      border_width: 1,
      border_color: 0x0000ff,
      corner_radius: 8,
      text_color: 0xffffff,
      text_size: 20,
    }"
    @click="onClick"
  >
    {{ label }}
  </div>
</template>
