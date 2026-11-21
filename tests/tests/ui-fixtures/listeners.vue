<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const log = ref("");
const note = (s) => () => (log.value += s);
const halt = (e) => {
  log.value += "halt ";
  e.stopImmediatePropagation();
};
</script>

<template>
  <div :style="{ width: 300, height: 300 }" @click="log += 'parent '">
    <div id="all" :style="{ width: 100, height: 40 }" :onClick="[note('a '), note('b ')]" />
    <div
      id="halted"
      :style="{ width: 100, height: 40 }"
      :onClick="[note('a '), halt, note('c ')]"
    />
    <div id="log">{{ log }}</div>
  </div>
</template>
