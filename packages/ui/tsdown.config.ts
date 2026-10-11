import { defineConfig } from "tsdown";

export default defineConfig({
  entry: ["src/index.ts"],
  format: "esm",
  platform: "neutral",
  target: "es2022",
  dts: false, // declarations are emitted by tsc (TypeScript 7 native) for exact fidelity
  sourcemap: true,
  clean: true,
  treeshake: true,
  // Mark the bundle as a client module so it works inside React Server Component apps (Next.js App Router).
  banner: { js: '"use client";' },
  deps: {
    neverBundle: [/^react/, /^radix-ui/, /^@radix-ui/, "sonner", "lucide-react", "class-variance-authority", "clsx", "tailwind-merge"],
  },
});
