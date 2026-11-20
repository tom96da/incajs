<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const count = ref(0);
</script>

<template>
  <div :style="{ width: 300, height: 200 }">
    <div id="box" :style="{ width: 100, height: 40 }" @click.once="count++" />
    <div id="count">{{ count }}</div>
  </div>
</template>
