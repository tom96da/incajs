<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const log = ref("");
const tab = ref("0");
const bare = ref(null);
</script>

<template>
  <div :style="{ width: 300, height: 400 }">
    <div
      id="target"
      :tabindex="tab"
      :style="{ width: 100, height: 40 }"
      @focus="log += 'target:focus '"
      @blur="log += 'target:blur '"
    />
    <div id="bare" ref="bare" :style="{ width: 100, height: 40 }" @focus="log += 'bare:focus '" />
    <div
      id="focus-bare"
      :style="{ width: 100, height: 40 }"
      @mousedown.prevent
      @click="
        bare.focus();
        log += 'called ';
      "
    />
    <div id="remove" :style="{ width: 100, height: 40 }" @mousedown.prevent @click="tab = null" />
    <div id="negative" :style="{ width: 100, height: 40 }" @mousedown.prevent @click="tab = -1" />
    <div id="empty" :style="{ width: 100, height: 40 }" />
    <div id="log">{{ log }}</div>
  </div>
</template>
