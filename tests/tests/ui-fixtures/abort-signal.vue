<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted, ref } from "vue";

const handled = ref("none");
const timedOut = ref("waiting");
const clicks = ref(0);
const counted = ref(null);

const controller = new AbortController();
controller.signal.onabort = () => {
  handled.value = "onabort";
};

const listenerController = new AbortController();
onMounted(() => {
  counted.value.addEventListener("click", () => clicks.value++, {
    signal: listenerController.signal,
  });
});

AbortSignal.timeout(50).addEventListener("abort", () => {
  timedOut.value = "timeout";
});
</script>

<template>
  <div :style="{ width: 300, height: 300 }">
    <div id="abort" :style="{ width: 100, height: 40 }" @click="controller.abort()" />
    <div id="counted" ref="counted" :style="{ width: 100, height: 40 }" />
    <div id="stop" :style="{ width: 100, height: 40 }" @click="listenerController.abort()" />
    <div id="clicks">{{ clicks }}</div>
    <div id="handled">{{ handled }}</div>
    <div id="timed-out">{{ timedOut }}</div>
  </div>
</template>
