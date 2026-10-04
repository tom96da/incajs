<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const seen = ref("none");
</script>

<template>
  <div :style="{ display: 'flex', flex_direction: 'row', width: 300, height: 200 }">
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
    <div id="seen">{{ seen }}</div>
  </div>
</template>
