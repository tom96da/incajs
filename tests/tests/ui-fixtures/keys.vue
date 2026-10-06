<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted, ref } from "@vue/runtime-core";

const log = ref("");
const a = ref(null);
const b = ref(null);
onMounted(() => a.value.focus());
</script>

<template>
  <div :style="{ width: 300, height: 300 }">
    <div
      id="a"
      tabindex="0"
      ref="a"
      :style="{ width: 100, height: 40 }"
      @keydown="log += `a:${$event.type}:${$event.key} `"
      @keyup="log += `a:${$event.type}:${$event.key} `"
      @click="log += 'a:click '"
      @focus="log += 'a:focus '"
      @blur="log += 'a:blur '"
    />
    <div
      id="b"
      tabindex="0"
      ref="b"
      :style="{ width: 100, height: 40 }"
      @keydown="log += `b:${$event.type}:${$event.key} `"
      @focus="log += 'b:focus '"
      @blur="log += 'b:blur '"
    />
    <div id="focus-b" :style="{ width: 100, height: 40 }" @click="b.focus()" />
    <div id="blur-b" :style="{ width: 100, height: 40 }" @click="b.blur()" />
    <div id="log">{{ log }}</div>
  </div>
</template>
