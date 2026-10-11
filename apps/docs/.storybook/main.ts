import { fileURLToPath } from "node:url";
import type { StorybookConfig } from "@storybook/react-vite";
import tailwindcss from "@tailwindcss/vite";

const uiSource = fileURLToPath(new URL("../../../packages/ui/src/index.ts", import.meta.url));

const config: StorybookConfig = {
  stories: ["../stories/**/*.mdx", "../stories/**/*.stories.@(ts|tsx)"],
  addons: ["@storybook/addon-docs", "@storybook/addon-a11y", "@storybook/addon-themes"],
  framework: { name: "@storybook/react-vite", options: {} },
  core: { disableTelemetry: true },
  // react-docgen (Babel-based) — TypeScript 7 is native and no longer exposes the JS API react-docgen-typescript needs.
  typescript: { reactDocgen: "react-docgen" },
  async viteFinal(cfg) {
    cfg.plugins = [...(cfg.plugins ?? []), tailwindcss()];
    // Point at component source for instant HMR while authoring; consumers use the built package.
    cfg.resolve = { ...cfg.resolve, alias: { ...(cfg.resolve?.alias ?? {}), "@nova/ui": uiSource } };
    return cfg;
  },
};

export default config;
