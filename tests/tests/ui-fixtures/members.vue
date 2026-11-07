<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const result = ref("");
const list = ref(null);
const second = ref(null);
const field = ref(null);
const off = ref(false);

function probe() {
  field.value.setAttribute("ID", "renamed");
  field.value.setAttribute("disabled", "false");
  let bad = "none";
  try {
    second.value.matches("div >");
  } catch (error) {
    bad = error.name;
  }
  let badQuery = "none";
  try {
    list.value.querySelector("div >");
  } catch (error) {
    badQuery = error.name;
  }
  result.value = JSON.stringify({
    tagName: list.value.tagName,
    attributeNames: list.value.getAttributeNames(),
    attribute: list.value.getAttribute("data-kind"),
    firstElementChild: list.value.firstElementChild === second.value.previousElementSibling,
    childElementCount: list.value.childElementCount,
    nextElementSibling: second.value.nextElementSibling === null,
    parentElement: second.value.parentElement === list.value,
    id: field.value.id,
    hasDisabled: field.value.hasAttribute("disabled"),
    matchesDisabled: field.value.matches(":disabled"),
    closest: second.value.closest(".card") === list.value,
    matches: second.value.matches(".card > li"),
    hover: second.value.matches(":hover"),
    syntaxError: bad,
    querySelector: list.value.querySelector("li + li") === second.value,
    querySelectorAllIds: Array.from(list.value.querySelectorAll("li"), (item) => item.id),
    querySelectorSelf: list.value.querySelector(".card"),
    querySelectorSyntaxError: badQuery,
    childrenLength: second.value.children.length,
    namedItemValue: list.value.attributes.getNamedItem("class").value,
    firstChildNodeValue: second.value.firstChild.nodeValue,
    childNodesLength: list.value.childNodes.length,
    nextSiblingNodeName: list.value.firstChild.nextSibling.nodeName,
    textContent: list.value.textContent,
  });
}
</script>

<template>
  <div :style="{ width: 300, height: 400 }">
    <ul ref="list" data-kind="people" class="card">
      <li id="first" :style="{ width: 50, height: 10 }">one</li>
      <li ref="second" id="second" :style="{ width: 50, height: 10 }">two</li>
    </ul>
    <button ref="field" id="field" :disabled="off" :style="{ width: 50, height: 20 }" />
    <div id="probe" :style="{ width: 50, height: 20 }" @mousedown.prevent @click="probe" />
    <div id="result">{{ result }}</div>
  </div>
</template>
