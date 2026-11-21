<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const result = ref("");

function onClick(e) {
  result.value = `${e.offsetX},${e.offsetY},${e.clientX},${e.clientY}`;
}
</script>

<template>
  <div :style="{ width: 300, height: 200, padding: 10 }">
    <div id="box" :style="{ width: 100, height: 50, border_width: 2 }" @click="onClick" />
    <div id="result">{{ result }}</div>
  </div>
</template>
