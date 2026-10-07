<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted, ref } from "@vue/runtime-core";

const log = ref("");
const off = ref(true);
const go = ref(null);
const note = (e, name) => {
  log.value += `${name}:${e.type}:${e.target === go.value.id ? "go" : "other"} `;
};
onMounted(() => go.value.focus());
</script>

<template>
  <div :style="{ width: 400, height: 500 }">
    <div id="wrap" :style="{ width: 200, height: 60 }" @click="note($event, 'wrap')">
      <button
        id="go"
        ref="go"
        :style="{ width: 100, height: 40 }"
        @click="
          log += `go:click:${$event.detail}:${$event.button}:${$event.clientX}:${$event.shiftKey} `
        "
        @keydown="log += `go:keydown:${$event.key} `"
        @keyup="log += `go:keyup:${$event.key} `"
        @focus="log += 'go:focus '"
      >
        go
      </button>
    </div>
    <div
      id="off-wrap"
      :style="{ width: 200, height: 60 }"
      @mousedown="log += 'off-wrap:mousedown '"
      @mouseup="log += 'off-wrap:mouseup '"
      @click="log += 'off-wrap:click '"
    >
      <button
        id="off"
        :disabled="off"
        :style="{ width: 100, height: 40 }"
        @mousedown="log += 'off:mousedown '"
        @mouseup="log += 'off:mouseup '"
        @click="log += 'off:click '"
        @focus="log += 'off:focus '"
      >
        off
      </button>
    </div>
    <div id="toggle" :style="{ width: 100, height: 40 }" @click="off = !off" />
    <div id="log">{{ log }}</div>
  </div>
</template>
