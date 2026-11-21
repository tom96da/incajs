<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const indexes = Array.from({ length: 30 }, (_, i) => i);
const focused = ref(-1);
</script>

<template>
  <div :style="{ display: 'flex', flex_direction: 'column', width: 300, height: 400, padding: 10 }">
    <!-- 200x100 content box inside 10px padding and 3px border -->
    <div
      id="box"
      :style="{ width: 226, height: 126, padding: 10, border_width: 3, overflow: 'scroll' }"
    >
      <div
        v-for="i in indexes"
        :id="`item-${i}`"
        :key="i"
        :tabindex="0"
        :style="{ width: 200, height: 30 }"
        @focus="focused = i"
      />
    </div>
    <div id="focused">{{ focused }}</div>
  </div>
</template>
