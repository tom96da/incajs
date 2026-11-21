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
    <div id="outer" :style="{ width: 200, height: 80 }" @click="log += 'outer '">
      <div
        id="inner"
        :style="{ width: 50, height: 40 }"
        @click="
          (e) => {
            log += 'inner ';
            e.stopPropagation();
          }
        "
      />
    </div>
    <div id="log">{{ log }}</div>
  </div>
</template>
