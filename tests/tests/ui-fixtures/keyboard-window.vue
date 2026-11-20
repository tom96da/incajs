<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted, ref } from "@vue/runtime-core";

import { window } from "incajs";

const log = ref("");
onMounted(() => {
  window.addEventListener("keydown", (e) => (log.value += `window:keydown:${e.key} `));
});
</script>

<template>
  <div :style="{ width: 300, height: 300 }">
    <div id="log">{{ log }}</div>
  </div>
</template>
