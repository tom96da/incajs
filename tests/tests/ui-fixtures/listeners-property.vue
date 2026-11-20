<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const count = ref(0);
const log = ref("");
const bump = () => {
  count.value += 1;
};
</script>

<template>
  <div :style="{ width: 300, height: 300 }" @click="log += `prevented:${$event.defaultPrevented} `">
    <div id="bump" :style="{ width: 100, height: 40 }" :onclick="bump" />
    <div id="cancel" :style="{ width: 100, height: 40 }" :onclick="() => false" />
    <div id="count">{{ count }}</div>
    <div id="log">{{ log }}</div>
  </div>
</template>
