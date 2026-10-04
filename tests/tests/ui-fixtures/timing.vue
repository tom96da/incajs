<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const open = ref(false);
const closes = ref(0);
const close = () => {
  open.value = false;
  closes.value++;
};
const onceOpen = ref(false);
const onceCloses = ref(0);
const onceClose = () => {
  onceOpen.value = false;
  onceCloses.value++;
};
</script>

<template>
  <div :style="{ width: 300, height: 300 }">
    <div id="plain" :style="{ width: 200, height: 80 }" :onClick="open ? close : null">
      <div id="plain-button" :style="{ width: 100, height: 40 }" @click="open = true" />
    </div>
    <div id="once" :style="{ width: 200, height: 80 }" :onClickOnce="onceOpen ? onceClose : null">
      <div id="once-button" :style="{ width: 100, height: 40 }" @click="onceOpen = true" />
    </div>
    <div id="state">{{ open }} {{ closes }}</div>
    <div id="once-state">{{ onceOpen }} {{ onceCloses }}</div>
  </div>
</template>
