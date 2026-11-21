<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const log = ref("");
const target = ref(null);
</script>

<template>
  <div :style="{ width: 300, height: 300 }">
    <div id="go" :style="{ width: 100, height: 40 }" @click="target.click()" />
    <div
      id="target"
      ref="target"
      :style="{ width: 100, height: 40 }"
      @click="log += `clicked:${$event.target === target} `"
    />
    <div id="log">{{ log }}</div>
  </div>
</template>
