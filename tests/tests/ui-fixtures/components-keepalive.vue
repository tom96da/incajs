<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { KeepAlive, ref } from "@vue/runtime-core";

import Tally from "./parts/tally.vue";

const showTally = ref(true);
</script>

<template>
  <div :style="{ width: 300, height: 300 }">
    <div id="switch" :style="{ width: 100, height: 40 }" @click="showTally = !showTally">
      Switch
    </div>
    <KeepAlive>
      <Tally v-if="showTally" />
      <div v-else id="other" :style="{ width: 100, height: 40 }">Other</div>
    </KeepAlive>
  </div>
</template>
