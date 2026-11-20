<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted, ref } from "@vue/runtime-core";

import { document, window } from "incajs";

const items = ref(["a", "b", "c"]);
const log = ref("");
const caps = ref("");
const menu = ref("");
const aux = ref("");
const dbl = ref("");
const cflags = ref("");
const got = ref("");
const els = {};
let kept = null;
const css = "#a, #b, #c { height: 40px } #b:hover { height: 50px }";

const nameOf = (node) => (node === document ? "document" : node.id);
const note = (e) => {
  if (e.target !== document && !e.target.id) return;
  log.value += `${e.type}@${nameOf(e.target)} `;
};
const noteCaps = (e) => {
  caps.value += `${e.type}@${nameOf(e.target)} `;
};
const attempt = (fn) => {
  try {
    fn();
  } catch (e) {
    caps.value += `${e.name} `;
  }
};

onMounted(() => {
  kept = els.a;
  for (const type of [
    "pointerdown",
    "pointerup",
    "pointermove",
    "pointerover",
    "pointerout",
    "pointerenter",
    "pointerleave",
    "mousedown",
    "mouseup",
    "mousemove",
    "mouseover",
    "mouseout",
    "mouseenter",
    "mouseleave",
    "click",
    "gotpointercapture",
    "lostpointercapture",
  ]) {
    document.addEventListener(type, note, true);
  }
  document.addEventListener("gotpointercapture", noteCaps, true);
  document.addEventListener("lostpointercapture", noteCaps, true);
  document.addEventListener(
    "contextmenu",
    (e) => {
      menu.value += `${e.type}@${nameOf(e.target)} `;
      e.preventDefault();
    },
    true,
  );
  document.addEventListener(
    "auxclick",
    (e) => (aux.value += `${e.type}@${nameOf(e.target)} `),
    true,
  );
  document.addEventListener(
    "dblclick",
    (e) => (dbl.value += `${e.type}@${nameOf(e.target)} `),
    true,
  );
  for (const type of ["gotpointercapture", "lostpointercapture"]) {
    els.a.addEventListener(type, (e) => {
      e.preventDefault();
      cflags.value += `${e.type}:${e.bubbles}:${e.cancelable}:${e.composed}:${e.defaultPrevented} `;
    });
  }
  els.a.addEventListener("gotpointercapture", (e) => {
    got.value += `${e.button},${e.buttons},${e.pressure} `;
  });
  window.addEventListener("keydown", (e) => {
    if (e.key === "F9") {
      log.value = caps.value = menu.value = aux.value = dbl.value = cflags.value = got.value = "";
    } else if (e.key === "r") {
      els.a.releasePointerCapture(1);
      caps.value += `has:${els.a.hasPointerCapture(1)} `;
    } else if (e.key === "x") {
      items.value = items.value.filter((i) => i !== "a");
    } else if (e.key === "m") {
      items.value = ["b", "c", "a"];
    } else if (e.key === "h") {
      caps.value += `has:${kept.hasPointerCapture(1)} `;
    } else if (e.key === "c") {
      els.b.setPointerCapture(1);
      caps.value += `has:${els.b.hasPointerCapture(1)} `;
    } else if (e.key === "s") {
      els.a.setPointerCapture(1);
      caps.value += `has:${els.a.hasPointerCapture(1)} `;
    } else if (e.key === "i") {
      attempt(() => els.a.setPointerCapture(7));
    } else if (e.key === "d") {
      attempt(() => kept.setPointerCapture(1));
    }
  });
});
</script>

<template>
  <div :style="{ width: 300, height: 400 }">
    <component is="style">{{ css }}</component>
    <div id="row" :style="{ display: 'flex', flex_direction: 'row' }">
      <div
        v-for="id in items"
        :id="id"
        :key="id"
        :ref="(el) => el && (els[id] = el)"
        :style="{ width: 100 }"
        @pointerdown="
          id === 'a'
            ? els.a.setPointerCapture($event.pointerId)
            : id === 'c' && els.b.setPointerCapture($event.pointerId)
        "
      />
    </div>
    <div id="log">{{ log }}</div>
    <div id="caps">{{ caps }}</div>
    <div id="menu">{{ menu }}</div>
    <div id="aux">{{ aux }}</div>
    <div id="dbl">{{ dbl }}</div>
    <div id="cflags">{{ cflags }}</div>
    <div id="got">{{ got }}</div>
  </div>
</template>
