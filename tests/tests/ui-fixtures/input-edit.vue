<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "vue";

const text = ref("");
const log = ref("");

function on(id) {
  const note = (e) => {
    const detail = e.type.startsWith("key") ? `:${e.key}` : "";
    const input = "inputType" in e ? `:${e.inputType}:${String(e.data)}` : "";
    log.value += `${id}:${e.type}${detail}${input} `;
  };
  return {
    keydown: note,
    keypress: note,
    beforeinput: note,
    input: note,
    change: note,
    keyup: note,
    blur: note,
  };
}
</script>

<template>
  <div :style="{ width: 300, height: 300, display: 'flex', flex_direction: 'column' }">
    <input id="f" v-model="text" v-on="on('f')" :style="{ width: 200, textSize: 10 }" />
    <input id="lock" v-on="on('lock')" @beforeinput.prevent :style="{ width: 200, textSize: 10 }" />
    <input id="nokd" v-on="on('nokd')" @keydown.prevent :style="{ width: 200, textSize: 10 }" />
    <input id="nokp" v-on="on('nokp')" @keypress.prevent :style="{ width: 200, textSize: 10 }" />
    <input id="ro" readonly value="abc" v-on="on('ro')" :style="{ width: 200, textSize: 10 }" />
    <input
      id="ro-nokp"
      readonly
      v-on="on('ro-nokp')"
      @keypress.prevent
      :style="{ width: 200, textSize: 10 }"
    />
    <div id="other" tabindex="0" :style="{ width: 100, height: 20 }" />
    <div id="value">{{ text }}</div>
    <div id="log">{{ log }}</div>
  </div>
</template>
