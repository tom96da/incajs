<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted, ref } from "vue";

import { window } from "incajs";

const log = ref("");
const win = ref("");
const box = ref(null);

function logKey(e) {
  log.value += `${e.type}:${e.key} `;
}

function logMenu(e) {
  log.value += `contextmenu:${e.button}:${e.buttons}:${e.detail}:${e.clientX}:${e.clientY}:${e.isTrusted} `;
}

onMounted(() => {
  box.value.focus();
  window.addEventListener("contextmenu", (e) => {
    win.value += `contextmenu@${e.target.id}:${e.clientX}:${e.clientY} `;
  });
});
</script>

<template>
  <div id="spacer" :style="{ width: 100, height: 40 }" />
  <div
    id="box"
    ref="box"
    tabindex="0"
    :style="{ width: 100, height: 40 }"
    @keydown="logKey"
    @keyup="logKey"
    @contextmenu="logMenu"
  />
  <div
    id="box-stop"
    tabindex="0"
    :style="{ width: 100, height: 40 }"
    @keydown="(logKey($event), $event.preventDefault())"
  />
  <div id="unfocus" :style="{ width: 100, height: 40 }" @mousedown.prevent="box.blur()" />
  <div id="log">{{ log }}</div>
  <div id="win">{{ win }}</div>
</template>
