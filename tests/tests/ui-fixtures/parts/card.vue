<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
defineProps({ title: String });
const emit = defineEmits(["change"]);
</script>

<template>
  <div id="card" :style="{ width: 200, height: 120, display: 'flex', flex_direction: 'column' }">
    <div id="title" :style="{ height: 20 }">{{ title }}</div>
    <div id="slot" :style="{ height: 40 }"><slot /></div>
    <div id="bump" :style="{ width: 50, height: 30 }" @click="emit('change', 5)">Bump</div>
  </div>
</template>
