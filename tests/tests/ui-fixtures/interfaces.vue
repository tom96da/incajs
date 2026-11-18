<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const result = ref("");
const box = ref(null);
const paragraph = ref(null);
const shown = ref(false);
const reattached = ref("");
const holder = ref(null);
let removedTop = null;

function removeTop() {
  holder.value.innerHTML = "<ul><li>x</li></ul>";
  removedTop = holder.value.firstChild;
  holder.value.removeChild(removedTop);
}

function reattach() {
  holder.value.appendChild(removedTop);
  reattached.value = holder.value.textContent;
}

function probe() {
  const fragment = document.createDocumentFragment();
  fragment.appendChild(document.createElement("div")).appendChild(document.createElement("p"));
  const fragmentQueryOne = fragment.querySelector("p")?.nodeName ?? null;
  const fragmentQueryAll = fragment.querySelectorAll("div, p").length;
  const list = document.createElement("ul");
  for (let i = 0; i < 1000; i++) list.appendChild(document.createElement("li"));
  const copy = list.cloneNode(true);
  const comment = box.value.lastChild;
  box.value.innerHTML = "<svg></svg>";
  const svg = box.value.firstChild;
  result.value = JSON.stringify({
    documentTag: Object.prototype.toString.call(document),
    refIsHtmlElement: box.value instanceof HTMLElement,
    textChildIsText: paragraph.value.firstChild instanceof Text,
    commentNodeType: comment.nodeType,
    commentIsCharacterData: comment instanceof CharacterData,
    commentIsElement: comment instanceof Element,
    svgIsElement: svg instanceof Element,
    svgIsHtmlElement: svg instanceof HTMLElement,
    fragmentIsDocumentFragment: document.createDocumentFragment() instanceof DocumentFragment,
    windowIsEventTarget: window instanceof EventTarget,
    fragmentFindsDescendants: [fragmentQueryOne, fragmentQueryAll],
    deepCloneChildTag: [copy.children.length, copy.children[999].tagName],
    textParentIsTheRef: paragraph.value.firstChild.parentElement === paragraph.value,
  });
}
</script>

<template>
  <div :style="{ width: 300, height: 400 }">
    <div ref="box" :style="{ width: 300, height: 100 }">
      <p ref="paragraph">hello</p>
      <span v-if="shown" />
    </div>
    <div id="probe" :style="{ width: 50, height: 20 }" @mousedown.prevent @click="probe" />
    <div id="result">{{ result }}</div>
    <div ref="holder" />
    <div id="removeTop" :style="{ width: 50, height: 20 }" @mousedown.prevent @click="removeTop" />
    <div id="reattach" :style="{ width: 50, height: 20 }" @mousedown.prevent @click="reattach" />
    <div id="reattached">{{ reattached }}</div>
  </div>
</template>
