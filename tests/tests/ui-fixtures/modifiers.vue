<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted, ref } from "@vue/runtime-core";

const log = ref("");
const field = ref(null);
onMounted(() => field.value.focus());
</script>

<template>
  <div :style="{ width: 300, height: 300 }">
    <div id="outer" :style="{ width: 200, height: 80 }" @click="log += 'outer '">
      <div id="inner" :style="{ width: 50, height: 40 }" @click.stop="log += 'inner '" />
    </div>
    <div
      id="field"
      ref="field"
      :style="{ width: 100, height: 40 }"
      @keydown.enter="log += 'enter '"
    />
    <div id="log">{{ log }}</div>
  </div>
</template>
