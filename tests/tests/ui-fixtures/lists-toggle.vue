<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const shown = ref(false);
</script>

<template>
  <div :style="{ display: 'flex', flex_direction: 'column' }">
    <div id="button" :style="{ width: 100, height: 40 }" @click="shown = !shown">Toggle</div>
    <div v-if="shown" id="panel" :style="{ width: 100, height: 40 }">Panel</div>
    <div v-else id="empty" :style="{ width: 100, height: 20 }">Empty</div>
  </div>
</template>
