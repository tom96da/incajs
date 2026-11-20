<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted, ref } from "@vue/runtime-core";

import { window } from "incajs";

const log = ref("");
const comp = ref("");
const value = ref("");
const lockedValue = ref("");
const keepValue = ref("");
const flags = ref("");
const disabled = ref(false);
const keep = ref(null);

const composition = (e) => {
  const entry = `${e.type}:${e.data}`;
  log.value += `${entry} `;
  comp.value += `${entry} `;
};
const compositionEnd = (e) => {
  composition(e);
  value.value = e.target.value;
};
const input = (e) => {
  log.value += `${e.type}:${e.inputType}:${e.data}:${e.isComposing}:${e.cancelable} `;
};
const edited = (e) => {
  input(e);
  value.value = e.target.value;
};
const flagged = (e) => {
  e.preventDefault();
  flags.value += `${e.type}:${e.bubbles}:${e.cancelable}:${e.composed}:${e.defaultPrevented} `;
};

onMounted(() => {
  window.addEventListener("keydown", (e) => {
    if (e.key === "F3") keepValue.value = keep.value.value;
    if (e.key === "F8") disabled.value = true;
    if (e.key === "F9") {
      log.value = "";
      comp.value = "";
      value.value = "";
      flags.value = "";
    }
  });
});
</script>

<template>
  <div :style="{ width: 300, height: 300, display: 'flex', flex_direction: 'column' }">
    <input
      id="field"
      value="ab"
      :disabled="disabled"
      :style="{ width: 200, textSize: 10 }"
      @compositionstart="composition"
      @compositionupdate="composition"
      @compositionend="compositionEnd"
      @beforeinput="input"
      @input="edited"
      @keydown="log += `${$event.type}:${$event.key}:${$event.isComposing} `"
      @change="log += `${$event.type} `"
      @blur="log += `${$event.type} `"
    />
    <input
      id="locked"
      value="ab"
      :style="{ width: 200, textSize: 10 }"
      @beforeinput.prevent
      @input="lockedValue = $event.target.value"
    />
    <input
      id="keep"
      ref="keep"
      value="ab"
      :style="{ width: 200, textSize: 10 }"
      @compositionend.prevent
    />
    <input
      id="flag"
      value="ab"
      :style="{ width: 200, textSize: 10 }"
      @compositionstart="flagged"
      @compositionupdate="flagged"
      @compositionend="flagged"
    />
    <div id="other" tabindex="0" :style="{ width: 100, height: 20 }" />
    <div id="comp">{{ comp }}</div>
    <div id="log">{{ log }}</div>
    <div id="value">{{ value }}</div>
    <div id="locked-value">{{ lockedValue }}</div>
    <div id="keep-value">{{ keepValue }}</div>
    <div id="flags">{{ flags }}</div>
  </div>
</template>
