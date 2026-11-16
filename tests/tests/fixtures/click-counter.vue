<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<!--
  click_counter. The style block and label suffix below are substituted at test time.
-->
<script setup>
import { computed, ref } from "@vue/runtime-core";

const clicks = ref(0);
const label = computed(() =>
  clicks.value === 0 ? "Click me!" : `Clicked ${clicks.value} time(s)__LABEL_SUFFIX__`,
);

console.log("[e2e] mounted");
</script>

<template>
  <div
    @click="clicks++"
    :style="{
      __STYLE__,
    }"
  >
    {{ label }}
  </div>
</template>
