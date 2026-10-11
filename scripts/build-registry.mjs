// Generates a shadcn-compatible registry (registry/r/*.json) from @nova/ui source + @nova/tokens.
// Consumers:  npx shadcn add $REGISTRY_URL/nova-theme.json $REGISTRY_URL/button.json
import { mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

const REGISTRY_URL = (process.env.REGISTRY_URL ?? "https://nova.example.com/r").replace(/\/$/, "");
const UI = "packages/ui/src";
const OUT = "registry/r";
const ITEM_SCHEMA = "https://ui.shadcn.com/schema/registry-item.json";

const meta = {
  badge: "Status / count label with semantic variants.",
  button: "Action trigger with variants, sizes, loading and asChild.",
  card: "Surface for grouping related content.",
  checkbox: "Checkbox, Switch and RadioGroup form controls.",
  dialog: "Modal dialog with focus trap, overlay and animations.",
  display: "Avatar, Separator and Kbd.",
  "dropdown-menu": "Menu of actions with checkbox/radio items and submenus.",
  feedback: "Alert, Toaster/toast (Sonner) and Skeleton.",
  field: "Label + Field: automatic label/description/error ARIA wiring.",
  input: "Input and auto-growing Textarea.",
  overlay: "Tooltip and Popover.",
  select: "Accessible select built on Radix.",
  spinner: "Loading indicator.",
  tabs: "Tabbed navigation.",
  theme: "ThemeProvider, useTheme and a no-flash init script.",
};

/** Rewrite package-internal imports to shadcn project aliases. */
function rewrite(src) {
  return src
    .replaceAll('from "../lib/cn"', 'from "@/lib/utils"')
    .replaceAll('from "../lib/focus"', 'from "@/lib/focus"')
    .replace(/from "\.\/([\w-]+)"/g, 'from "@/components/ui/$1"')
    // Nova-prefixed variables → shadcn variable names.
    .replaceAll("var(--nv-radius-lg)", "var(--radius)")
    .replaceAll("var(--nv-font-family-sans)", "var(--font-sans)")
    .replaceAll("var(--nv-color-", "var(--");
}

const npmDeps = (src) =>
  [...src.matchAll(/from "([^".][^"]*)"/g)]
    .map((m) => m[1])
    .filter((d) => !d.startsWith("@/") && !d.startsWith("react"))
    .map((d) => (d.startsWith("@") ? d.split("/").slice(0, 2).join("/") : d.split("/")[0]));

const localDeps = (src) => [...src.matchAll(/from "\.\/([\w-]+)"/g)].map((m) => m[1]);

await rm(OUT, { recursive: true, force: true });
await mkdir(OUT, { recursive: true });

const items = [];

// lib/focus — shared interaction classes
const focusSrc = await readFile(`${UI}/lib/focus.ts`, "utf8");
items.push({
  name: "nova-focus",
  type: "registry:lib",
  title: "Nova focus utilities",
  description: "Shared focus/invalid/disabled class strings used by every Nova control.",
  files: [{ path: "lib/focus.ts", type: "registry:lib", content: focusSrc }],
});

// Components
for (const file of (await readdir(`${UI}/components`)).filter((f) => f.endsWith(".tsx")).sort()) {
  const name = path.basename(file, ".tsx");
  const src = await readFile(`${UI}/components/${file}`, "utf8");
  const registryDependencies = [
    ...(src.includes("../lib/cn") ? ["utils"] : []),
    ...(src.includes("../lib/focus") ? [`${REGISTRY_URL}/nova-focus.json`] : []),
    ...localDeps(src).map((d) => `${REGISTRY_URL}/${d}.json`),
  ];
  items.push({
    name,
    type: "registry:ui",
    title: name.replace(/(^|-)(\w)/g, (_, s, c) => (s ? " " : "") + c.toUpperCase()),
    description: meta[name] ?? "",
    dependencies: [...new Set(npmDeps(src))],
    registryDependencies,
    files: [{ path: `components/ui/${file}`, type: "registry:ui", content: rewrite(src) }],
  });
}

// Theme from tokens: shadcn cssVars (light/dark) + Tailwind v4 theme extensions for extra semantic colors.
const light = JSON.parse(await readFile("packages/tokens/dist/json/light.json", "utf8"));
const darkOverrides = JSON.parse(await readFile("packages/tokens/dist/json/dark.json", "utf8"));
const resolve = (map, p, seen = 0) => {
  const v = String(map[p]?.value ?? "");
  const ref = v.match(/^\{(.+)\}$/);
  return ref && seen < 5 ? resolve(map, ref[1], seen + 1) : v;
};
const pick = (map) =>
  Object.fromEntries(Object.keys(map).filter((k) => k.startsWith("color.")).map((k) => [k.slice(6), resolve(map, k)]));
const lightVars = { ...pick(light), radius: String(light["radius.lg"].value) };
const darkVars = pick({ ...light, ...darkOverrides });
const extra = ["success", "success-foreground", "warning", "warning-foreground", "info", "info-foreground"];
items.push({
  name: "nova-theme",
  type: "registry:theme",
  title: "Nova theme",
  description: "Nova semantic colors (OKLCH, WCAG AA-gated), radius and status colors for any shadcn project.",
  cssVars: {
    theme: {
      ...Object.fromEntries(extra.map((k) => [`color-${k}`, `var(--${k})`])),
      "ease-standard": "cubic-bezier(0.2, 0, 0, 1)",
      "ease-emphasized": "cubic-bezier(0.3, 0, 0, 1)",
      "animate-fade-in": "nv-fade-in 200ms cubic-bezier(0, 0, 0.2, 1)",
      "animate-fade-out": "nv-fade-out 120ms cubic-bezier(0.4, 0, 1, 1) forwards",
      "animate-scale-in": "nv-scale-in 200ms cubic-bezier(0.3, 0, 0, 1)",
      "animate-scale-out": "nv-scale-out 120ms cubic-bezier(0.4, 0, 1, 1) forwards",
    },
    light: lightVars,
    dark: darkVars,
  },
  css: {
    "@keyframes nv-fade-in": { from: { opacity: "0" } },
    "@keyframes nv-fade-out": { to: { opacity: "0" } },
    "@keyframes nv-scale-in": { from: { opacity: "0", scale: "0.96" } },
    "@keyframes nv-scale-out": { to: { opacity: "0", scale: "0.96" } },
  },
});

for (const item of items) {
  await writeFile(`${OUT}/${item.name}.json`, `${JSON.stringify({ $schema: ITEM_SCHEMA, ...item }, null, 2)}\n`);
}
await writeFile(
  "registry/registry.json",
  `${JSON.stringify(
    {
      $schema: "https://ui.shadcn.com/schema/registry.json",
      name: "nova",
      homepage: REGISTRY_URL.replace(/\/r$/, ""),
      items: items.map(({ files, cssVars, ...rest }) => ({
        ...rest,
        ...(files ? { files: files.map(({ content, ...f }) => ({ ...f, path: `r/${rest.name}.json` })) } : {}),
      })),
    },
    null,
    2,
  )}\n`,
);
console.log(`✓ Registry built: ${items.length} items → ${OUT}/ (base URL ${REGISTRY_URL})`);
