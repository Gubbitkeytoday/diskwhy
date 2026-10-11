// Builds diskwhy-cli (CLI + MCP server) in release mode and copies it to src-tauri/binaries/ so the Tauri bundler
// ships it inside the installer (tauri.conf.json -> bundle.resources). Run by `beforeBuildCommand` and `pnpm cli:build`.
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, openSync, readSync, closeSync, statSync } from "node:fs";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const exe = process.platform === "win32" ? "diskwhy-cli.exe" : "diskwhy-cli";

const build = spawnSync("cargo", ["build", "--release", "-p", "diskwhy-cli"], { cwd: appDir, stdio: "inherit", shell: process.platform === "win32" });
if (build.error || build.status !== 0) {
  console.error(`\n[build-cli] cargo build failed${build.error ? `: ${build.error.message}` : ` (exit ${build.status})`}`);
  process.exit(build.status ?? 1);
}

const targetDir = process.env.CARGO_TARGET_DIR ? (isAbsolute(process.env.CARGO_TARGET_DIR) ? process.env.CARGO_TARGET_DIR : resolve(appDir, process.env.CARGO_TARGET_DIR)) : join(appDir, "target");
const src = join(targetDir, "release", exe);
const dest = join(appDir, "src-tauri", "binaries", "diskwhy-cli.exe");

try {
  if (statSync(src).size === 0) throw new Error("file is empty");
} catch (e) {
  console.error(`[build-cli] expected CLI at ${src} but it is unusable: ${e.message}`);
  process.exit(1);
}
if (process.platform === "win32") {
  // A Windows executable starts with "MZ"; refuse to bundle anything else.
  const fd = openSync(src, "r");
  const head = Buffer.alloc(2);
  readSync(fd, head, 0, 2, 0);
  closeSync(fd);
  if (head.toString("latin1") !== "MZ") {
    console.error(`[build-cli] ${src} is not a Windows executable`);
    process.exit(1);
  }
}

mkdirSync(dirname(dest), { recursive: true });
copyFileSync(src, dest);
console.log(`[build-cli] ${src} -> ${dest} (${(statSync(dest).size / 1024).toFixed(0)} KB)`);
