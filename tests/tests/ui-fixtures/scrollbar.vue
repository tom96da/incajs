<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted, ref } from "@vue/runtime-core";

import { document, window } from "incajs";

const box = ref(null);
const hbox = ref(null);
const log = ref("");
const moves = ref("");
const ups = ref("");
const caps = ref("");
const scrolled = ref("");
const scrollTop = ref("");
const atEnd = ref("");
const hAtEnd = ref("");
const active = ref("");
let cancel = false;
let capture = false;

const logTypes = [
  "pointerdown",
  "mousedown",
  "pointerup",
  "mouseup",
  "click",
  "pointerover",
  "pointerout",
  "pointerenter",
  "pointerleave",
  "mouseover",
  "mouseout",
  "mouseenter",
  "mouseleave",
  "contextmenu",
];
const moveTypes = ["pointermove", "mousemove"];
const upTypes = ["pointerup", "mouseup", "click"];
const capTypes = ["gotpointercapture", "lostpointercapture"];

const listen = (types, add) => {
  for (const type of types) {
    document.addEventListener(
      type,
      (e) => {
        if (e.target.id) add(`${e.type}@${e.target.id} `);
      },
      true,
    );
  }
};

const near = (a, b) => Math.abs(a - b) < 0.01;

onMounted(() => {
  listen(logTypes, (entry) => (log.value += entry));
  listen(moveTypes, (entry) => (moves.value += entry));
  listen(upTypes, (entry) => (ups.value += entry));
  listen(capTypes, (entry) => (caps.value += entry));
  document.addEventListener(
    "pointerup",
    () => (active.value = document.activeElement?.id ?? ""),
    true,
  );
  box.value.addEventListener("pointerdown", (e) => {
    if (cancel) e.preventDefault();
    if (capture) box.value.setPointerCapture(1);
  });
  box.value.addEventListener("scroll", () => (scrolled.value = "yes"));
  window.addEventListener("keydown", (e) => {
    if (e.key === "p") cancel = true;
    if (e.key === "k") capture = true;
    if (e.key === "e") {
      scrollTop.value = String(box.value.scrollTop);
      atEnd.value = String(
        near(box.value.scrollTop, box.value.scrollHeight - box.value.clientHeight),
      );
      hAtEnd.value = String(
        near(hbox.value.scrollLeft, hbox.value.scrollWidth - hbox.value.clientWidth),
      );
    }
  });
  window.addEventListener("keydown", (e) => {
    if (e.key === "F9") {
      log.value = "";
      moves.value = "";
      ups.value = "";
      caps.value = "";
    }
  });
});
</script>

<template>
  <div :style="{ width: 300, height: 700 }">
    <div
      id="above"
      :style="{ width: 100, height: 20 }"
      @pointerdown="$event.currentTarget.setPointerCapture(1)"
    />
    <div id="box" ref="box" :style="{ width: 100, height: 100, overflow_y: 'scroll' }">
      <div id="tall" :style="{ width: 80, height: 1000 }" />
    </div>
    <input id="field" :style="{ width: 100, height: 20 }" />
    <div id="hbox" ref="hbox" :style="{ width: 100, height: 100, overflow_x: 'scroll' }">
      <div id="wide" :style="{ width: 1000, height: 80 }" />
    </div>
    <div id="both" :style="{ width: 100, height: 100, overflow: 'scroll' }">
      <div id="huge" :style="{ width: 1000, height: 1000 }" />
    </div>
    <div id="log">{{ log }}</div>
    <div id="moves">{{ moves }}</div>
    <div id="ups">{{ ups }}</div>
    <div id="caps">{{ caps }}</div>
    <div id="scrolled">{{ scrolled }}</div>
    <div id="scroll-top">{{ scrollTop }}</div>
    <div id="at-end">{{ atEnd }}</div>
    <div id="h-at-end">{{ hAtEnd }}</div>
    <div id="active">{{ active }}</div>
  </div>
</template>
