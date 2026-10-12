<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<!--
  Dispatches `ping` and then `pong` on `window`, one per `tick` dev
  notification. See hmr-callback-ids.test.mts.
-->
<script setup>
import { window } from "incajs";

const events = ["ping", "pong"];
const previousReceive = globalThis.__inca_dev__.receive;
globalThis.__inca_dev__.receive = (method, paramsJson) => {
  if (method !== "tick") return previousReceive?.(method, paramsJson);
  const name = events.shift();
  window.dispatchEvent(new CustomEvent(name));
  console.log("[e2e] dispatched " + name);
};

console.log("[e2e] mounted");
__inca_dev__.send("mounted", "{}");
</script>

<template>
  <div>ids</div>
</template>
