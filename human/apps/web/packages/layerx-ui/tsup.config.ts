import { defineConfig } from "tsup";

const shared = {
  format: ["esm", "cjs"] as const,
  dts: true,
  sourcemap: true,
  clean: false,
  external: ["react", "react-dom", "tailwindcss"],
  esbuildOptions(options: { jsx: string }) {
    options.jsx = "automatic";
  },
};

export default defineConfig([
  {
    ...shared,
    entry: ["src/index.ts"],
    banner: { js: '"use client";' },
  },
  {
    ...shared,
    entry: { cn: "src/lib/utils.ts" },
  },
]);
