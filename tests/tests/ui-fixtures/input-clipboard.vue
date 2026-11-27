<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted, ref } from "vue";

import { window } from "incajs";

const value = ref("abcd");
const log = ref("");
const data = ref("");
const locked = ref(null);
const lockedValue = ref("");

const note = (e) => (log.value += `${e.type}:${e.inputType}:${e.data} `);
const noteKey = (e) => (log.value += `${e.type}:${e.key} `);
const noteData = (e) => (data.value += `${e.type}:${JSON.stringify(e.data)} `);

onMounted(() => {
  window.addEventListener("keydown", (e) => {
    if (e.key === "F3") lockedValue.value = locked.value.value;
  });
  window.addEventListener("keydown", (e) => {
    if (e.key !== "F9") return;
    log.value = "";
    data.value = "";
  });
});
</script>

<template>
  <div :style="{ width: 300, height: 300, display: 'flex', flex_direction: 'column' }">
    <input
      id="field"
      v-model="value"
      :style="{ width: 200, textSize: 10 }"
      @beforeinput="
        note($event);
        noteData($event);
      "
      @input="
        note($event);
        noteData($event);
      "
    />
    <input
      id="ro"
      readonly
      value="abcd"
      :style="{ width: 200, textSize: 10 }"
      @keydown="noteKey"
      @beforeinput="note"
      @input="note"
    />
    <input
      id="locked"
      ref="locked"
      value="abcd"
      :style="{ width: 200, textSize: 10 }"
      @beforeinput.prevent
    />
    <div id="log">{{ log }}</div>
    <div id="data">{{ data }}</div>
    <div id="value">{{ value }}</div>
    <div id="locked-value">{{ lockedValue }}</div>
  </div>
</template>
