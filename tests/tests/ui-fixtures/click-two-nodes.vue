<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted, ref } from "vue";

import { document, window } from "incajs";

const btn = ref(null);
const log = ref("");
const seq = ref("");
const buttons = ref("");
const css = "#a { height: 40px } #a:active { height: 60px }";

const note = (e) => {
  log.value += `${e.type}@${e.target.id || "root"} `;
};
const noteSeq = (e) => {
  seq.value += `${e.type}@${e.currentTarget.id} `;
};

onMounted(() => {
  for (const type of ["pointerdown", "mousedown", "pointerup", "mouseup", "click"]) {
    document.addEventListener(type, note, true);
  }
  for (const type of [
    "pointerdown",
    "mousedown",
    "focus",
    "focusin",
    "keydown",
    "keyup",
    "pointerup",
    "mouseup",
    "click",
  ]) {
    btn.value.addEventListener(type, noteSeq);
  }
  window.addEventListener("keydown", (e) => {
    if (e.key === "F9") {
      log.value = "";
      seq.value = "";
    }
  });
});
</script>

<template>
  <div :style="{ width: 300, height: 400 }">
    <component is="style">{{ css }}</component>
    <div id="row" :style="{ display: 'flex', flex_direction: 'row' }">
      <div id="a" :style="{ width: 100 }">
        <div id="a1" :style="{ width: 50, height: 20 }" />
      </div>
      <div
        id="b"
        :style="{ width: 100, height: 40 }"
        @mousemove="buttons = String($event.buttons)"
      />
    </div>
    <button id="btn" ref="btn" :style="{ width: 80, height: 30 }" />
    <div id="log">{{ log }}</div>
    <div id="seq">{{ seq }}</div>
    <div id="buttons">{{ buttons }}</div>
  </div>
</template>
