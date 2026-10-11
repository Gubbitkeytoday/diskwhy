const ref = (p) => `{palette.${p}}`;
const light = {
  background: ["white", "Page background"],
  foreground: ["neutral.950", "Default text"],
  card: ["white", "Raised surface (cards, panels)"],
  "card-foreground": ["neutral.950"],
  popover: ["white", "Floating surface (menus, popovers)"],
  "popover-foreground": ["neutral.950"],
  primary: ["brand.600", "Primary action / brand emphasis"],
  "primary-foreground": ["white"],
  secondary: ["neutral.100", "Secondary action"],
  "secondary-foreground": ["neutral.900"],
  muted: ["neutral.100", "Subdued surface"],
  "muted-foreground": ["neutral.600", "Secondary text, hints, placeholders"],
  accent: ["brand.50", "Hover / selected background"],
  "accent-foreground": ["brand.800"],
  destructive: ["red.600", "Destructive action / error"],
  "destructive-foreground": ["white"],
  success: ["green.700", "Success state"],
  "success-foreground": ["white"],
  warning: ["amber.400", "Warning state"],
  "warning-foreground": ["amber.950"],
  info: ["blue.600", "Informational state"],
  "info-foreground": ["white"],
  border: ["neutral.200", "Default border / divider"],
  input: ["neutral.300", "Form control border"],
  ring: ["brand.500", "Focus ring"],
  "chart-1": ["brand.600", "Data series 1 (primary)"],
  "chart-2": ["green.500", "Data series 2"],
  "chart-3": ["amber.500", "Data series 3"],
  "chart-4": ["blue.500", "Data series 4"],
  "chart-5": ["red.400", "Data series 5"],
};
const dark = {
  background: ["neutral.950"], foreground: ["neutral.50"],
  card: ["neutral.900"], "card-foreground": ["neutral.50"],
  popover: ["neutral.900"], "popover-foreground": ["neutral.50"],
  primary: ["brand.400"], "primary-foreground": ["brand.950"],
  secondary: ["neutral.800"], "secondary-foreground": ["neutral.50"],
  muted: ["neutral.800"], "muted-foreground": ["neutral.300"],
  accent: ["brand.900"], "accent-foreground": ["brand.100"],
  destructive: ["red.500"], "destructive-foreground": ["red.950"],
  success: ["green.500"], "success-foreground": ["green.950"],
  warning: ["amber.400"], "warning-foreground": ["amber.950"],
  info: ["blue.400"], "info-foreground": ["blue.950"],
  border: ["neutral.800"], input: ["neutral.700"], ring: ["brand.400"],
  "chart-1": ["brand.400"], "chart-2": ["green.400"], "chart-3": ["amber.400"], "chart-4": ["blue.400"], "chart-5": ["red.400"],
};
const build = (map, mode) => {
  const color = { $type: "color" };
  for (const [k, [p, d]] of Object.entries(map)) {
    color[k] = { $value: ref(p), ...(d ? { $description: d } : {}) };
  }
  return { $description: `Nova semantic color tokens — ${mode} mode. Names follow the shadcn/ui convention so the whole shadcn ecosystem works out of the box.`, color };
};
const fs = await import("node:fs");
fs.writeFileSync("src/semantic.light.tokens.json", JSON.stringify(build(light, "light"), null, 2) + "\n");
fs.writeFileSync("src/semantic.dark.tokens.json", JSON.stringify(build(dark, "dark"), null, 2) + "\n");
