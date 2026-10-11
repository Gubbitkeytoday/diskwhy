// WCAG 2.2 contrast gate for every semantic color pair, in both themes. Fails the build on regressions.
import { readFile } from "node:fs/promises";
import { parse, wcagContrast } from "culori";

const AA_TEXT = 4.5;
const AA_UI = 3;

const pairs = [
  ["foreground", "background", AA_TEXT],
  ["card-foreground", "card", AA_TEXT],
  ["popover-foreground", "popover", AA_TEXT],
  ["primary-foreground", "primary", AA_TEXT],
  ["secondary-foreground", "secondary", AA_TEXT],
  ["muted-foreground", "muted", AA_TEXT],
  ["muted-foreground", "background", AA_TEXT],
  ["accent-foreground", "accent", AA_TEXT],
  ["destructive-foreground", "destructive", AA_TEXT],
  ["success-foreground", "success", AA_TEXT],
  ["warning-foreground", "warning", AA_TEXT],
  ["info-foreground", "info", AA_TEXT],
  ["primary", "background", AA_UI],
  ["destructive", "background", AA_UI],
  ["ring", "background", AA_UI],
];

const load = async (f) => JSON.parse(await readFile(`dist/json/${f}.json`, "utf8"));
const light = await load("light");
const dark = { ...light, ...(await load("dark")) };

let failed = 0;
for (const [mode, map] of [["light", light], ["dark", dark]]) {
  console.log(`\n${mode.toUpperCase()}`);
  for (const [fg, bg, min] of pairs) {
    const ratio = wcagContrast(parse(map[`color.${fg}`].value), parse(map[`color.${bg}`].value));
    const ok = ratio >= min;
    if (!ok) failed++;
    console.log(`  ${ok ? "✓" : "✗"} ${fg.padEnd(24)} on ${bg.padEnd(12)} ${ratio.toFixed(2).padStart(5)}:1  (min ${min})`);
  }
}
if (failed) {
  console.error(`\n✗ ${failed} contrast pair(s) below WCAG AA`);
  process.exit(1);
}
console.log("\n✓ All semantic color pairs pass WCAG 2.2 AA");
