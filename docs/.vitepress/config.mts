// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { defineConfig } from "vitepress";
import { groupIconMdPlugin, groupIconVitePlugin } from "vitepress-plugin-group-icons";
import llmstxt from "vitepress-plugin-llms";
import type { HeadConfig } from "vitepress";

import packageJson from "../../packages/core/package.json" with { type: "json" };
import structuredData from "./structured-data.json" with { type: "json" };

const incaVersion = packageJson.version;
const commitRef = process.env.COMMIT_REF?.slice(0, 8) || "dev";

const siteName = "Incarnative.js";
const siteUrl = packageJson.homepage.replace(/\/$/, "");
const repoUrl = packageJson.repository.url.replace(/^git\+/, "").replace(/\.git$/, "");

/** The canonical URL of a page, in the `cleanUrls` form the site serves. */
const pageUrl = (relativePath: string) =>
  `${siteUrl}/${relativePath.replace(/\.md$/, "").replace(/(^|\/)index$/, "$1")}`;

export default defineConfig({
  title: siteName,
  description:
    "A GPU-native, Webview-free desktop application framework powered by GPUI and QuickJS.",
  base: "/",
  lastUpdated: true,
  sitemap: {
    hostname: `${siteUrl}/`,
  },
  transformHead({ pageData, title, description }) {
    if (pageData.relativePath === "404.md") return [];
    const isHome = pageData.relativePath === "index.md";
    const url = pageUrl(pageData.relativePath);
    const head: HeadConfig[] = [
      ["link", { rel: "canonical", href: url }],
      ["meta", { property: "og:type", content: isHome ? "website" : "article" }],
      ["meta", { property: "og:site_name", content: siteName }],
      ["meta", { property: "og:title", content: title }],
      ["meta", { property: "og:description", content: description }],
      ["meta", { property: "og:url", content: url }],
      ["meta", { name: "twitter:card", content: "summary" }],
    ];
    if (isHome) {
      const softwareSourceCode = {
        ...structuredData,
        name: siteName,
        description,
        url: `${siteUrl}/`,
        codeRepository: repoUrl,
        sameAs: [repoUrl, `https://www.npmjs.com/package/${packageJson.name}`],
        version: incaVersion,
      };
      head.push([
        "script",
        { type: "application/ld+json" },
        JSON.stringify(softwareSourceCode).replace(/</g, "\\u003c"),
      ]);
    }
    return head;
  },
  themeConfig: {
    nav: [
      { text: "Guide", link: "/guide/", activeMatch: "^/guide/" },
      { text: "Reference", link: "/reference/configuration", activeMatch: "^/reference/" },
      { text: "Roadmap", link: "/roadmap" },
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
          items: [
            { text: "How it Works", link: "/guide/how-it-works" },
            { text: "Getting Started", link: "/guide/" },
          ],
        },
        {
          text: "Guide",
          items: [
            { text: "HMR", link: "/guide/hmr" },
            { text: "Application Window", link: "/guide/window" },
            { text: "Building for Production", link: "/guide/build" },
            { text: "Troubleshooting", link: "/guide/troubleshooting" },
          ],
        },
      ],
      "/reference/": [
        {
          text: "Reference",
          items: [
            { text: "CLI", link: "/reference/cli" },
            { text: "Configuration", link: "/reference/configuration" },
            { text: "Elements & Styles", link: "/reference/elements" },
            { text: "Events", link: "/reference/events" },
            { text: "Console", link: "/reference/console" },
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
    plugins: [
      groupIconVitePlugin(),
      llmstxt({
        domain: siteUrl,
        title: siteName,
        description:
          "Incarnative.js (inca) is an ultra-lightweight, Webview-free desktop application framework powered by GPUI, QuickJS, and custom renderers.",
      }),
    ],
  },
  cleanUrls: true,
});
