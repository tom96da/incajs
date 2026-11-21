<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const tag = ref("on");
const toggle = () => (tag.value = tag.value === undefined ? "on" : undefined);
</script>

<template>
  <div :style="{ width: 400, height: 100 }">
    <div id="box" :tag="tag" :style="{ width: 100, height: 40 }" @click="toggle" />
  </div>
</template>
