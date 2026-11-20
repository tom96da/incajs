<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const tall = ref(false);
</script>

<template>
  <div :style="{ width: 400, height: 200 }">
    <div id="box" :style="{ width: 100, height: tall ? 80 : 40 }" @click="tall = !tall" />
  </div>
</template>
