<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const holder = ref(null);

const fill = () => {
  holder.value.innerHTML = '<div id="made" style="width: 80px; height: 20px"></div>';
};
</script>

<template>
  <div :style="{ width: 300, height: 300 }">
    <div id="fill" :style="{ width: 60, height: 20 }" @click="fill" />
    <div ref="holder" id="holder" />
  </div>
</template>
