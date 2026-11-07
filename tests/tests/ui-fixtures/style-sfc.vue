<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import StyleChild from "./parts/style-child.vue";

import "./parts/shared.css";
</script>

<template>
  <div :style="{ display: 'flex', flex_direction: 'row' }">
    <div id="plain" class="plain" :style="{ height: 10 }" />
    <div id="scoped" class="item" :style="{ height: 10 }" />
    <StyleChild />
    <div id="imported" class="imported" :style="{ height: 10 }" />
    <div id="order" class="order" :style="{ height: 10 }" />
    <div id="module" :class="$style.sized" :style="{ height: 10 }" />
  </div>
</template>

<style>
@import "./parts/imported.css";

.plain {
  width: 30px;
}

.order {
  width: 22px;
}
</style>

<style scoped>
.item {
  width: 40px;
}
</style>

<style module>
.sized {
  width: 70px;
}
</style>
