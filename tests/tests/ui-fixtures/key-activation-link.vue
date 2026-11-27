<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted, ref } from "vue";

const log = ref("");
const lnk = ref(null);
const anchors = [
  { id: "bare", tabindex: 0 },
  { id: "dis", disabled: true },
  { id: "stop" },
  { id: "stop-press" },
];
const cancels = { stop: "keydown", "stop-press": "keypress" };

function note(e) {
  const id = e.currentTarget.id;
  if (e.type === "click") {
    log.value += `${id}:click:${e.detail}:${e.button}:${e.shiftKey}:${e.isTrusted} `;
    return;
  }
  log.value += `${id}:${e.type}:${e.key === " " ? "Space" : e.key} `;
  if (cancels[id] === e.type) {
    e.preventDefault();
  }
}

const handlers = { onKeydown: note, onKeypress: note, onKeyup: note, onClick: note };

onMounted(() => lnk.value.focus());
</script>

<template>
  <div :style="{ width: 300, height: 500 }">
    <a
      id="lnk"
      ref="lnk"
      href="#"
      :style="{ display: 'block', width: 100, height: 40 }"
      v-bind="handlers"
      >lnk</a
    >
    <a
      v-for="a in anchors"
      :id="a.id"
      :key="a.id"
      :href="a.id === 'bare' ? undefined : '#'"
      :tabindex="a.tabindex"
      :disabled="a.disabled"
      :style="{ display: 'block', width: 100, height: 40 }"
      v-bind="handlers"
      >{{ a.id }}</a
    >
    <div id="log">{{ log }}</div>
  </div>
</template>
