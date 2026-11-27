<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted, ref } from "vue";

const log = ref("");
const holder = ref(null);
const svg = "http://www.w3.org/2000/svg";

function note(e) {
  const id = e.currentTarget.id;
  if (e.type === "click") {
    log.value += `${id}:click:${e.detail}:${e.button} `;
    return;
  }
  log.value += `${id}:${e.type}:${e.key === " " ? "Space" : e.key} `;
}

onMounted(() => {
  const made = [
    ["a", "svg-href", "href", "#"],
    ["a", "svg-xlink", "xlink:href", "#"],
    ["button", "svg-btn", "tabindex", "0"],
  ];
  for (const [tag, id, name, value] of made) {
    const node = document.createElementNS(svg, tag);
    node.setAttribute("id", id);
    node.setAttribute(name, value);
    for (const type of ["keydown", "keypress", "keyup", "click"]) {
      node.addEventListener(type, note);
    }
    holder.value.appendChild(node);
  }
});
</script>

<template>
  <div :style="{ width: 300, height: 300 }">
    <div id="holder" ref="holder" />
    <div id="log">{{ log }}</div>
  </div>
</template>
