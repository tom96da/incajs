<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const log = ref("");
const named = ref(true);
const plain = ref(null);
const five = ref(null);
const toggled = ref(null);

const read = () => {
  log.value = [plain, five, toggled].map((r) => JSON.stringify(r.value.id)).join(" ");
};
const assign = () => {
  plain.value.id = 7;
  five.value.id = null;
  read();
};
</script>

<template>
  <div :style="{ width: 300, height: 300 }">
    <div id="read" :style="{ width: 60, height: 20 }" @click="read" />
    <div id="assign" :style="{ width: 60, height: 20 }" @click="assign" />
    <div id="drop" :style="{ width: 60, height: 20 }" @click="named = false" />
    <div ref="plain" />
    <div ref="five" :id="5" />
    <div ref="toggled" :id="named ? 'named' : null" />
    <div id="log">{{ log }}</div>
  </div>
</template>
