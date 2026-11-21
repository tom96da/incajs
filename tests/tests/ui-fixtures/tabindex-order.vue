<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const log = ref("");
const indexes = [3, 1, 0, 0, -1, 2];
</script>

<template>
  <div :style="{ width: 300, height: 400 }">
    <div
      v-for="(index, i) in indexes"
      :id="`t${i}`"
      :key="i"
      :tabindex="index"
      :style="{ width: 100, height: 20 }"
      @focus="log += `t${i} `"
    />
    <div id="log">{{ log }}</div>
  </div>
</template>
