<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const text = ref("");
</script>

<template>
  <div :style="{ width: 300, height: 200, display: 'flex', flex_direction: 'column' }">
    <input id="field" v-model="text" :style="{ width: 200, textSize: 10 }" />
    <div id="mirror">{{ text }}</div>
    <div id="clear" :style="{ width: 40, height: 20 }" @click="text = ''" />
  </div>
</template>
