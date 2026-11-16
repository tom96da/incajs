<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const handled = ref("none");
const timedOut = ref("waiting");

const controller = new AbortController();
controller.signal.onabort = () => {
  handled.value = "onabort";
};

AbortSignal.timeout(50).addEventListener("abort", () => {
  timedOut.value = "timeout";
});
</script>

<template>
  <div :style="{ width: 300, height: 200 }">
    <div id="abort" :style="{ width: 100, height: 40 }" @click="controller.abort()" />
    <div id="handled">{{ handled }}</div>
    <div id="timed-out">{{ timedOut }}</div>
  </div>
</template>
