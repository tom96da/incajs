<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const log = ref("");
const onWrap = (event) => {
  log.value += event.target === event.currentTarget ? "wrap " : "delegated ";
};
</script>

<template>
  <div :style="{ width: 300, height: 300 }">
    <div id="outer" :style="{ width: 200, height: 80 }" @click.self="log += 'self '">
      <div id="inner" :style="{ width: 50, height: 40 }" />
    </div>
    <div id="wrap" :style="{ width: 100, height: 40 }" @click="onWrap">
      <div id="kid" :style="{ width: 20, height: 20 }" />
    </div>
    <div id="log">{{ log }}</div>
  </div>
</template>
