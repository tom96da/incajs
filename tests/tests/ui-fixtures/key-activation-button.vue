<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted, ref } from "@vue/runtime-core";

const log = ref("");
const selfOff = ref(false);
const go = ref(null);
const buttons = ["down-stop", "press-stop", "up-stop", "other", "self-off"];
const cancels = { "down-stop": "keydown", "press-stop": "keypress", "up-stop": "keyup" };

function note(e) {
  const id = e.currentTarget.id;
  if (e.type === "click") {
    log.value += `${id}:click:${e.detail}:${e.button}:${e.ctrlKey}:${e.altKey}:${e.shiftKey}:${e.metaKey} `;
    return;
  }
  log.value += `${id}:${e.type}:${e.key === " " ? "Space" : e.key} `;
  if (cancels[id] === e.type) {
    e.preventDefault();
  }
  if (id === "self-off" && e.type === "keydown") {
    selfOff.value = true;
  }
}

const handlers = { onKeydown: note, onKeypress: note, onKeyup: note, onClick: note };

onMounted(() => go.value.focus());
</script>

<template>
  <div :style="{ width: 300, height: 500 }">
    <button id="go" ref="go" :style="{ width: 100, height: 40 }" v-bind="handlers">go</button>
    <button
      v-for="id in buttons"
      :id="id"
      :key="id"
      :disabled="id === 'self-off' && selfOff"
      :style="{ width: 100, height: 40 }"
      v-bind="handlers"
    >
      {{ id }}
    </button>
    <div id="plain" tabindex="0" :style="{ width: 100, height: 40 }" v-bind="handlers" />
    <div id="log">{{ log }}</div>
  </div>
</template>
