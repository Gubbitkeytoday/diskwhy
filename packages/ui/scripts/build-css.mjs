// Ships two CSS entry points:
//  - dist/styles.css : for apps that already use Tailwind v4 (smallest output, tree-shaken by the app)
//  - dist/nova.css   : precompiled, zero-config stylesheet for apps without Tailwind
import { execFileSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";

const src = await readFile("src/styles.css", "utf8");
// The Tailwind entry scans the compiled components that sit next to it in dist/.
await writeFile("dist/styles.css", `${src}\n@source "./";\n`);

const require = createRequire(import.meta.url);
const cli = path.join(path.dirname(require.resolve("@tailwindcss/cli/package.json")), "dist", "index.mjs");
execFileSync(process.execPath, [cli, "-i", "src/standalone.css", "-o", "dist/nova.css", "--minify"], { stdio: "inherit" });

const size = (await readFile("dist/nova.css")).byteLength;
console.log(`✓ @nova/ui CSS built (nova.css ${(size / 1024).toFixed(1)} KB minified)`);
