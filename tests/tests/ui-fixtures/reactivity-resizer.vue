<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const wide = ref(false);
</script>

<template>
  <div :style="{ width: 400, height: 100 }">
    <div id="box" :style="{ width: wide ? 200 : 100, height: 40 }" @click="wide = !wide" />
  </div>
</template>
