<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "vue";

const seen = ref("none");
</script>

<template>
  <div :style="{ display: 'flex', flex_direction: 'row', width: 500, height: 200 }">
    <div
      id="box"
      :style="{ width: 100, height: 100, overflow: 'scroll' }"
      @wheel="seen = $event.deltaY"
    >
      <div id="first" :style="{ width: 80, height: 60 }" />
      <div id="second" :style="{ width: 80, height: 60 }" />
      <div id="third" :style="{ width: 80, height: 60 }" />
    </div>
    <div
      id="locked"
      :style="{ width: 100, height: 100, overflow: 'scroll' }"
      @wheel="(e) => e.preventDefault()"
    >
      <div id="locked-first" :style="{ width: 80, height: 60 }" />
      <div id="locked-second" :style="{ width: 80, height: 60 }" />
      <div id="locked-third" :style="{ width: 80, height: 60 }" />
    </div>
    <div
      id="wide"
      :style="{
        display: 'flex',
        flex_direction: 'row',
        width: 100,
        height: 100,
        overflow: 'scroll',
      }"
    >
      <div id="wide-first" :style="{ width: 80, height: 60, flex_shrink: 0 }" />
      <div id="wide-second" :style="{ width: 80, height: 60, flex_shrink: 0 }" />
      <div id="wide-third" :style="{ width: 80, height: 60, flex_shrink: 0 }" />
    </div>
    <div id="grid" :style="{ width: 100, height: 100, overflow: 'scroll', flex_shrink: 0 }">
      <div id="cell" :style="{ width: 1000, height: 1000 }" />
    </div>
    <div id="seen">{{ seen }}</div>
  </div>
</template>
