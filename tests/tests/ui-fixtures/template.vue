<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted, ref } from "@vue/runtime-core";

const holder = ref(null);
const source = ref(null);
const result = ref("");

onMounted(() => {
  holder.value.content.appendChild(source.value.cloneNode(false));
});

function read() {
  result.value = holder.value.content.firstChild.id;
}
</script>

<template>
  <div :style="{ display: 'flex', flex_direction: 'row' }">
    <component is="template" ref="holder">
      <div id="own" :style="{ height: 10, width: 10 }" />
    </component>
    <div id="shown" ref="source" :style="{ height: 10, width: 10 }" />
    <div id="read" :style="{ height: 10, width: 10 }" @click="read" />
    <div id="result">{{ result }}</div>
  </div>
</template>
