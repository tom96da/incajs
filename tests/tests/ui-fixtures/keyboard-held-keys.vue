<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted, ref } from "vue";

import { window } from "incajs";

const log = ref("");
const fromWindow = ref("");
const value = ref("");
const clicks = ref(0);

const note = (e) =>
  (log.value +=
    `${e.type}:${e.key}:${e.repeat}:${e.shiftKey}:${e.ctrlKey}:${e.altKey}:${e.metaKey}:` +
    `${e.keyCode} `);

onMounted(() => {
  window.addEventListener("keydown", (e) => {
    fromWindow.value += `window:${e.key} `;
  });
  window.addEventListener("keydown", (e) => {
    if (e.key !== "F9") return;
    log.value = "";
    fromWindow.value = "";
    clicks.value = 0;
  });
});
</script>

<template>
  <div :style="{ width: 300, height: 300, display: 'flex', flex_direction: 'column' }">
    <div
      id="box"
      tabindex="0"
      :style="{ width: 100, height: 40 }"
      @keydown="note"
      @keypress="note"
      @keyup="note"
    />
    <input id="field" v-model="value" :style="{ width: 200, textSize: 10 }" />
    <button id="btn" :style="{ width: 80, height: 30 }" @click="clicks++">go</button>
    <div id="log">{{ log }}</div>
    <div id="window">{{ fromWindow }}</div>
    <div id="mirror">{{ value }}</div>
    <div id="clicks">{{ clicks }}</div>
  </div>
</template>
