<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const log = ref("");
</script>

<template>
  <div :style="{ width: 300, height: 200 }">
    <div
      id="box"
      :style="{ width: 100, height: 40 }"
      @click.left="log += 'left '"
      @click.ctrl="log += 'ctrl '"
      @click.middle="log += 'middle '"
      @click.right="log += 'right '"
      @contextmenu.prevent="log += `menu:${$event.defaultPrevented} `"
    />
    <div id="log">{{ log }}</div>
  </div>
</template>
