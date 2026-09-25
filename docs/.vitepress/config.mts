// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { defineConfig } from "vitepress";
import { groupIconMdPlugin, groupIconVitePlugin } from "vitepress-plugin-group-icons";

import packageJson from "../../packages/core/package.json" with { type: "json" };

const incaVersion = packageJson.version;
const commitRef = process.env.COMMIT_REF?.slice(0, 8) || "dev";

export default defineConfig({
  title: "Incarnative.js",
  description:
    "A GPU-native, Webview-free desktop application framework powered by GPUI and QuickJS.",
  base: "/incajs/",
  lastUpdated: true,
  themeConfig: {
    nav: [
      { text: "Guide", link: "/guide/", activeMatch: "^/guide/" },
      { text: "Reference", link: "/reference/configuration", activeMatch: "^/reference/" },
      {
        text: `v${incaVersion}`,
        items: [
          {
            text: "Changelog",
            link: "https://github.com/tom96da/incajs/blob/main/CHANGELOG.md",
          },
        ],
      },
    ],
    sidebar: {
      "/guide/": [
        {
          text: "Introduction",
          items: [{ text: "Getting Started", link: "/guide/" }],
        },
        {
          text: "Guide",
          items: [
            { text: "CLI", link: "/guide/cli" },
            { text: "Application Window", link: "/guide/window" },
            { text: "Building for Production", link: "/guide/build" },
          ],
        },
      ],
      "/reference/": [
        {
          text: "Reference",
          items: [
            { text: "Configuration", link: "/reference/configuration" },
            { text: "Events", link: "/reference/events" },
            { text: "Error Codes", link: "/reference/errors" },
          ],
        },
      ],
    },
    socialLinks: [
      { icon: "github", link: "https://github.com/tom96da/incajs" },
      { icon: "npm", link: "https://npmjs.com/package/incajs" },
    ],
    footer: {
      message: "Released under the MIT or Apache-2.0 license.",
      copyright: `© 2026 tom96da. (${commitRef})`,
    },
    search: {
      provider: "local",
    },
  },
  markdown: {
    config(md) {
      md.use(groupIconMdPlugin);
    },
  },
  vite: {
    plugins: [groupIconVitePlugin()],
  },
  cleanUrls: true,
});
