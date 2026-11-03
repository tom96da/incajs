<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const shown = ref(true);
const css = "#b { width: 60px } .box { width: 40px }";
</script>

<template>
  <div :style="{ display: 'flex', flex_direction: 'row' }">
    <component is="style" v-if="shown">{{ css }}</component>
    <div id="a" class="box" :style="{ height: 10 }" />
    <div id="b" class="box" :style="{ height: 10 }" />
    <div id="remove" :style="{ height: 10, width: 10 }" @click="shown = false" />
  </div>
</template>
