<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const box = ref(null);

function resize() {
  box.value.style.width = "40px";
}
</script>

<template>
  <div :style="{ display: 'flex', flex_direction: 'row' }">
    <div id="box" ref="box" class="box" :style="{ height: 10 }" />
    <div id="probe" :style="{ height: 10, width: 10 }" @mousedown.prevent @click="resize" />
  </div>
</template>

<style>
.box {
  width: 20px;
}
</style>
