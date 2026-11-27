<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted } from "vue";

onMounted(() => {
  document.addEventListener("wheel", (e) => e.preventDefault());
  window.addEventListener("wheel", (e) => e.preventDefault());
});
</script>

<template>
  <div id="box" :style="{ width: 100, height: 60, overflow: 'auto' }">
    <div id="content" :style="{ width: 80, height: 120 }" />
  </div>
</template>
