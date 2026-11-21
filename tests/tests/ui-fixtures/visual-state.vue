<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
const css =
  "input { width: 40px } input:focus { width: 60px } " +
  ".box { width: 40px } .box:hover { width: 60px }";
</script>

<template>
  <div :style="{ display: 'flex', flex_direction: 'column' }">
    <component is="style">{{ css }}</component>
    <input id="first" :style="{ height: 10 }" />
    <input id="second" :style="{ height: 10 }" />
    <div :style="{ height: 20 }" />
    <div id="box" class="box" :style="{ height: 10 }" />
  </div>
</template>
