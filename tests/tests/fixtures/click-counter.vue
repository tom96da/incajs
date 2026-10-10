<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<!--
  click_counter, driven by a `tick` dev notification instead of a real
  pointer click — see hmr-quickjs-state.test.mts for why. The style block
  and label suffix below are substituted at test time.
-->
<script setup>
import { computed, ref, watch } from "@vue/runtime-core";

const clicks = ref(0);
const label = computed(() =>
  clicks.value === 0 ? "Click me!" : `Clicked ${clicks.value} time(s)__LABEL_SUFFIX__`,
);

const previousReceive = globalThis.__inca_dev__.receive;
globalThis.__inca_dev__.receive = (method, paramsJson) => {
  if (method === "tick") clicks.value += 1;
  else previousReceive?.(method, paramsJson);
};

watch(clicks, (v) => console.log(`[e2e] clicks=${v}`));
console.log("[e2e] mounted");
__inca_dev__.send("mounted", "{}");
</script>

<template>
  <div
    :style="{
      __STYLE__,
    }"
  >
    {{ label }}
  </div>
</template>
