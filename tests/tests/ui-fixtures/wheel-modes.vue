<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<template>
  <div>
    <div id="box" :style="{ width: 100, height: 100, overflow_y: 'scroll' }">
      <div id="first" :style="{ width: 80, height: 60 }" />
      <div id="second" :style="{ width: 80, height: 60 }" />
      <div id="third" :style="{ width: 80, height: 60 }" />
    </div>
    <div id="grid" :style="{ width: 100, height: 100, overflow: 'scroll' }">
      <div id="cell" :style="{ width: 1000, height: 1000 }" />
    </div>
  </div>
</template>
