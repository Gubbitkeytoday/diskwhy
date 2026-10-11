const steps = [50,100,200,300,400,500,600,700,800,900,950];
const L = [0.985,0.967,0.928,0.872,0.775,0.665,0.545,0.47,0.4,0.34,0.26];
const F = [0.12,0.25,0.45,0.7,0.9,1,0.95,0.85,0.7,0.55,0.4];
const hues = {
  neutral: { h: 262, c: 0.014, desc: "Cool neutral for surfaces, text and borders" },
  brand:   { h: 277, c: 0.215, desc: "Primary brand hue (indigo-violet)" },
  red:     { h: 25,  c: 0.215, desc: "Danger / destructive" },
  amber:   { h: 70,  c: 0.17,  desc: "Warning", lshift: 0.06 },
  green:   { h: 152, c: 0.165, desc: "Success" },
  blue:    { h: 245, c: 0.18,  desc: "Info" },
};
const r = (n) => Math.round(n * 1000) / 1000;
const palette = { white: { $type: "color", $value: "oklch(1 0 0)" }, black: { $type: "color", $value: "oklch(0 0 0)" } };
for (const [name, { h, c, desc, lshift = 0 }] of Object.entries(hues)) {
  const group = { $description: desc };
  steps.forEach((s, i) => {
    const l = Math.min(0.99, L[i] + (i > 0 && i < 8 ? lshift : 0));
    group[s] = { $type: "color", $value: `oklch(${r(l)} ${r(c * F[i])} ${h})` };
  });
  palette[name] = group;
}
const out = {
  $description: "Nova primitive tokens. Raw values only — never use these directly in components; use semantic tokens.",
  palette,
  radius: {
    $type: "dimension",
    none: { $value: "0px" }, xs: { $value: "0.125rem" }, sm: { $value: "0.25rem" }, md: { $value: "0.375rem" },
    lg: { $value: "0.5rem" }, xl: { $value: "0.75rem" }, "2xl": { $value: "1rem" }, full: { $value: "9999px" },
  },
  space: { base: { $type: "dimension", $value: "0.25rem", $description: "Spacing unit. Tailwind `p-4` = 4 × base = 1rem." } },
  font: {
    family: {
      $type: "fontFamily",
      sans: { $value: ["Inter Variable", "Inter", "IBM Plex Sans Thai", "Noto Sans Thai", "ui-sans-serif", "system-ui", "sans-serif"], $description: "UI text. Latin = Inter, Thai = IBM Plex Sans Thai." },
      display: { $value: ["Inter Variable", "Inter", "IBM Plex Sans Thai Looped", "IBM Plex Sans Thai", "ui-sans-serif", "system-ui", "sans-serif"] },
      mono: { $value: ["JetBrains Mono Variable", "JetBrains Mono", "ui-monospace", "SFMono-Regular", "Menlo", "monospace"] },
    },
    weight: {
      $type: "fontWeight",
      regular: { $value: 400 }, medium: { $value: 500 }, semibold: { $value: 600 }, bold: { $value: 700 },
    },
    size: {
      $type: "dimension",
      xs: { $value: "0.75rem" }, sm: { $value: "0.875rem" }, base: { $value: "1rem" }, lg: { $value: "1.125rem" },
      xl: { $value: "1.25rem" }, "2xl": { $value: "1.5rem" }, "3xl": { $value: "1.875rem" }, "4xl": { $value: "2.25rem" }, "5xl": { $value: "3rem" },
    },
    "line-height": {
      $type: "dimension",
      $description: "Paired with font.size. Taller than Latin-only systems so Thai vowels/tone marks never clip.",
      xs: { $value: "1.25rem" }, sm: { $value: "1.5rem" }, base: { $value: "1.625rem" }, lg: { $value: "1.75rem" },
      xl: { $value: "1.875rem" }, "2xl": { $value: "2.125rem" }, "3xl": { $value: "2.5rem" }, "4xl": { $value: "2.875rem" }, "5xl": { $value: "3.5rem" },
    },
  },
  shadow: {
    $type: "shadow",
    xs: { $value: { color: "oklch(0.2 0.02 262 / 0.06)", offsetX: "0px", offsetY: "1px", blur: "2px", spread: "0px" } },
    sm: { $value: [
      { color: "oklch(0.2 0.02 262 / 0.08)", offsetX: "0px", offsetY: "1px", blur: "3px", spread: "0px" },
      { color: "oklch(0.2 0.02 262 / 0.06)", offsetX: "0px", offsetY: "1px", blur: "2px", spread: "-1px" } ] },
    md: { $value: [
      { color: "oklch(0.2 0.02 262 / 0.08)", offsetX: "0px", offsetY: "4px", blur: "8px", spread: "-2px" },
      { color: "oklch(0.2 0.02 262 / 0.06)", offsetX: "0px", offsetY: "2px", blur: "4px", spread: "-2px" } ] },
    lg: { $value: [
      { color: "oklch(0.2 0.02 262 / 0.10)", offsetX: "0px", offsetY: "12px", blur: "24px", spread: "-4px" },
      { color: "oklch(0.2 0.02 262 / 0.06)", offsetX: "0px", offsetY: "4px", blur: "8px", spread: "-4px" } ] },
    xl: { $value: [
      { color: "oklch(0.2 0.02 262 / 0.14)", offsetX: "0px", offsetY: "24px", blur: "48px", spread: "-12px" } ] },
  },
  duration: {
    $type: "duration",
    instant: { $value: "0ms" }, fast: { $value: "120ms" }, normal: { $value: "200ms" }, slow: { $value: "320ms" },
  },
  easing: {
    $type: "cubicBezier",
    standard: { $value: [0.2, 0, 0, 1] }, emphasized: { $value: [0.3, 0, 0, 1] },
    in: { $value: [0.4, 0, 1, 1] }, out: { $value: [0, 0, 0.2, 1] },
  },
};
process.stdout.write(JSON.stringify(out, null, 2) + "\n");
