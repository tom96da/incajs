<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

import Card from "./parts/card.vue";

const title = ref("First");
const total = ref(0);
</script>

<template>
  <div :style="{ width: 300, height: 300 }">
    <Card :title="title" @change="(n) => (total += n)">
      <div id="slotted" :style="{ width: 60, height: 20 }">Slotted</div>
    </Card>
    <div id="rename" :style="{ width: 100, height: 30 }" @click="title = 'Second'">Rename</div>
    <div id="total">{{ total }}</div>
  </div>
</template>
